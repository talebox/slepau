use std::{future::ready, path::PathBuf};

use axum::{
	routing::{get_service, MethodRouter},
	Router,
};
use hyper::StatusCode;
use tower_http::services::{ServeDir, ServeFile};

// use crate::utils::{WEB_DIST, WEB_DIST_LOGIN};

pub fn index_service(dir: &str, index: Option<&str>) -> MethodRouter {
	let assets_dir = PathBuf::from(dir);
	let index_file = assets_dir.join(index.unwrap_or("index.html"));
	get_service(ServeFile::new(index_file).precompressed_br().precompressed_gzip())
		.handle_error(|_| ready(StatusCode::INTERNAL_SERVER_ERROR))
}
pub fn assets_service(dir: &str, default_to_index: bool) -> MethodRouter {
	get_service(my_serve_dir(dir, default_to_index)).handle_error(|_| ready(StatusCode::INTERNAL_SERVER_ERROR))
}
pub fn my_serve_dir(dir: &str, default_to_index: bool) -> ServeDir {
	ServeDir::new(PathBuf::from(dir))
		.precompressed_br()
		.precompressed_gzip()
		.append_index_html_on_directories(default_to_index)
}
pub fn static_routes() -> Router {
	Router::new()
		// .nest_service("/app", index_service(WEB_DIST.as_str(), Some("index.html")))
		// .nest_service(
		// 	"/login",
		// 	assets_service(WEB_DIST_LOGIN.as_str(), true)
		// 		.fallback_service(index_service(WEB_DIST_LOGIN.as_str(), Some("index.html"))),
		// )
		// .nest_service(
		// 	"/web",
		// 	get_service(my_serve_dir(WEB_DIST.as_str(), false).fallback(my_serve_dir(WEB_DIST_LOGIN.as_str(), false)))
		// 		.handle_error(|_| ready(StatusCode::INTERNAL_SERVER_ERROR)),
		// )
		// .fallback_service(index_service(WEB_DIST.as_str(), Some("home.html")))
}
