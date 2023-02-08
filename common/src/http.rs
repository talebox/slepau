use std::{future::ready, path::PathBuf};

use axum::routing::{get_service, MethodRouter};
use hyper::StatusCode;
use tower_http::services::{ServeDir, ServeFile};

pub fn index_service(dir: &str, index: Option<&str>) -> MethodRouter {
	let assets_dir = PathBuf::from(dir);
	let index_file = assets_dir.join(index.unwrap_or("index.html"));
	get_service(ServeFile::new(index_file)).handle_error(|_| ready(StatusCode::INTERNAL_SERVER_ERROR))
}
pub fn assets_service(dir: &str) -> MethodRouter {
	let assets_dir = PathBuf::from(dir);
	get_service(ServeDir::new(assets_dir).precompressed_br().precompressed_gzip())
		.handle_error(|_| ready(StatusCode::INTERNAL_SERVER_ERROR))
}
