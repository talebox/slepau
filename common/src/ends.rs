pub async fn index_service_user(
	Extension(claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, impl IntoResponse> {
	fn get_src() -> Option<String> {
		std::fs::read_to_string(PathBuf::from(WEB_DIST.as_str()).join("index.html")).ok()
	}

	let src;
	// If we're debugging, get home every time
	if cfg!(debug_assertions) {
		src = get_src();
	} else {
		lazy_static! {
			static ref HOME: Option<String> = get_src();
		}
		src = HOME.to_owned();
	}

	src
		.as_ref()
		.map(|home| {
			(
				[(header::CONTENT_TYPE, "text/html")],
				home.replace("_USER_", serde_json::to_string(&claims).unwrap().as_str()),
			)
		})
		.ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}