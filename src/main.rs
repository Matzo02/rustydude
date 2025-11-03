use axum::{
    extract::{Multipart, Path, State},
    http::{HeaderMap, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt},
};

const UPLOAD_DIR: &str = "upload";
const AUTH_KEY: &str = "mysecretkey123";

#[derive(Clone)]
struct AppState {
    key: String,
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState {
        key: AUTH_KEY.to_string(),
    });

    let app = Router::new()
        .route("/", get(|| async { "RustDrop Server Running" }))
        .route("/file/:name", get(serve_file))
        .route("/upload", post(upload_file))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("✅ Server running at http://localhost:3000");
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

async fn serve_file(Path(filename): Path<String>) -> impl IntoResponse {
    let mut path = std::env::current_dir().unwrap();
    path.push("shared");
    path.push(&filename); // ✅ append filename to path

    match File::open(&path).await {
        Ok(mut file) => {
            let mut buffer = Vec::new();
            if let Err(_) = file.read_to_end(&mut buffer).await {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file").into_response();
            }

            Response::builder()
                .header("Content-Type", "application/octet-stream")
                .header(
                    "Content-Disposition",
                    format!("attachment; filename=\"{}\"", filename),
                )
                .body(buffer.into())
                .unwrap()
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}
// I love fighting against my own compiler.
async fn upload_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> impl IntoResponse {
    //  Auth check using header
    if let Some(key) = headers.get("Authorization") {
        if key != state.key.as_str() {
            return (StatusCode::UNAUTHORIZED, "Invalid API key").into_response();
        }
    } else {
        return (StatusCode::UNAUTHORIZED, "Missing Authorization header").into_response();
    }

    //  Ensure upload folder exists
    if let Err(e) = fs::create_dir_all(UPLOAD_DIR).await {
        eprintln!("Failed to create upload directory: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Server error").into_response();
    }

    while let Some(field) = multipart.next_field().await.unwrap() {
        let filename = field.file_name().unwrap_or("upload.bin").to_string();

        // Restrict allowed file types
        let allowed_extensions = ["txt", "png", "jpg", "jpeg", "pdf"];
        if let Some(ext) = std::path::Path::new(&filename).extension() {
            if !allowed_extensions.contains(&ext.to_str().unwrap_or_default()) {
                return (StatusCode::BAD_REQUEST, "File type not allowed").into_response();
            }
        } else {
            return (StatusCode::BAD_REQUEST, "File has no extension").into_response();
        }

        let data = field.bytes().await.unwrap();

        let mut path = PathBuf::from(UPLOAD_DIR);
        path.push(&filename);

        let mut file = match File::create(&path).await {
            Ok(f) => f,
            Err(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create file").into_response()
            }
        };

        if let Err(_) = file.write_all(&data).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to save file").into_response();
        }

        println!("File Uploaded: {}", filename);
    }

    (StatusCode::OK, "File uploaded successfully!").into_response()
}
