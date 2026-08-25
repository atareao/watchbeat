use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use include_dir::{include_dir, Dir};
use std::path::Path;

static FRONTEND_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../frontend/dist");

pub async fn serve_embedded(path: &str) -> Response {
    // Strip leading slash
    let clean_path = path.trim_start_matches('/');

    // Try exact match
    if let Some(file) = FRONTEND_DIR.get_file(clean_path) {
        let body = file.contents().to_vec();
        let mime = mime_type(clean_path);
        return Response::builder()
            .header("Content-Type", mime)
            .body(Body::from(body))
            .unwrap();
    }

    // Try index.html for SPA fallback (accept anything with no extension)
    if let Some(file) = FRONTEND_DIR.get_file("index.html") {
        let body = file.contents().to_vec();
        return Response::builder()
            .header("Content-Type", "text/html")
            .body(Body::from(body))
            .unwrap();
    }

    // Not found
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not found"))
        .unwrap()
}

fn mime_type(path: &str) -> &'static str {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "html" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}
