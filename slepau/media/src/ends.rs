use auth::UserClaims;
use axum::{
	body::StreamBody,
	extract::{Path, RawBody},
	http::header,
	response::IntoResponse, Extension, Json,
};
use common::utils::WEB_DIST;
use hyper::{StatusCode, body::to_bytes};


use log::info;
use proquint::Quintable;
use serde::{Deserialize, Serialize};
use std::{
	hash::{Hasher, Hash}, collections::hash_map::DefaultHasher, path::PathBuf,
};

use lazy_static::lazy_static;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use media::{MEDIA_FOLDER, MatcherType};


pub async fn home_service(
	// Extension(db): Extension<LockedAtomic<DB>>,
	Extension(claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, impl IntoResponse> {

	fn get_src() -> Option<String> {
		std::fs::read_to_string(PathBuf::from(WEB_DIST.as_str()).join("home.html")).ok()
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
				home
					.replace("_USER_", serde_json::to_string(&claims).unwrap().as_str()),
			)
		})
		.ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn media_get(Path(id): Path<String>) -> Result<impl IntoResponse, impl IntoResponse> {
	let path = std::path::Path::new(MEDIA_FOLDER.as_str());
	let path = path.join(id);

	let mut file = match tokio::fs::File::open(&path).await {
		Ok(file) => file,
		Err(err) => return Err((StatusCode::NOT_FOUND, format!("File not found: {}", err))),
	};

	let mut buf = [0u8; 64];
	if let Ok(_size) = file.read(&mut buf).await {
		file.rewind().await.unwrap(); // Reset the counter to start of file
		let _type = infer::get(&buf);

		// // convert the `AsyncRead` into a `Stream`
		let stream = ReaderStream::new(file);

		// // convert the `Stream` into an `axum::body::HttpBody`
		let body = StreamBody::new(stream);

		let headers = [
			(
				header::CONTENT_TYPE,
				match _type {
					Some(_type) => _type.mime_type(),
					None => "text/plain",
				},
			),
			(header::CACHE_CONTROL, "max-age=31536000"), // Makes browser cache for a year
		];
		Ok((headers, body))
	} else {
		Err((StatusCode::NO_CONTENT, "Error reading file?".to_string()))
	}
}

#[derive(Serialize)]
pub struct MediaPostResponse {
	id: String,

	#[serde(with = "MatcherType", rename = "type")]
	_type: infer::MatcherType,
}

/// - [ ] Uploading to POST `api/media` will
/// - create `data/media` if it doesn't exist
/// - save under `data/media/<32bit_hash_proquint>`, return error `<hash> exists` if exists already, else, return `<hash>`.
pub async fn media_post(
	// Extension(db): Extension<DB>,
	// Extension(cache): Extension<LockedAtomic<Cache>>,
	// Extension(user_claims): Extension<UserClaims>,
	body: RawBody,
) -> Result<impl IntoResponse, impl IntoResponse> {
	let path = std::path::Path::new(MEDIA_FOLDER.as_str());
	if !path.exists() {
		tokio::fs::create_dir(&path).await.unwrap();
		info!("Created media folder");
	}

	let mut body = to_bytes(body.0).await.unwrap();
	let mut id;
	{
		// Calculate hash
		let mut hasher = DefaultHasher::new();
		body.hash(&mut hasher);
		id = hasher.finish().to_quint();
	}

	// Do conversion if necessary
	let _type = infer::get(&body);
	let mut matcher_type = _type.map(|v| v.matcher_type()).unwrap_or(infer::MatcherType::Custom);

	// Don't perform conversion/file write if we have this id.
	let mut create = true;
	// {
	// 	let cache = cache.read().unwrap();
	// 	if let Some(media_item) = cache.media.get(&id) {
	// 		// let mut cache_item = cache_item.clone();
	// 		// If we have a reference to a new conversion, make that the current id
	// 		if let MediaEntry::Ref(id_cache) = media_item {
	// 			if let Some(media_item) = cache.media.get(id_cache) {
	// 				id = id_cache.clone();

	// 				if let MediaEntry::Entry { user: _, _type } = media_item {
	// 					matcher_type = *_type;
	// 				} else {
	// 					error!(
	// 						"Media entry isn't Entry for {}? was referenced by {} that's weird",
	// 						id, id_cache
	// 					);
	// 				}
	// 			} else {
	// 				create = true;
	// 			}
	// 		}
	// 	} else {
	// 		create = true
	// 	}
	// }

	if create {
		if let Some(_type) = _type {
			match _type.matcher_type() {
				// infer::MatcherType::Image => {
				// 	if let Ok(img) = image::load_from_memory(&body) {
				// 		let mut _body = BufWriter::new(Cursor::new(vec![]));
				// 		info!("Converting image w:{},h:{} to .avif", img.width(), img.height());
				// 		img.write_to(&mut _body, image::ImageOutputFormat::Avif).unwrap();
				// 		info!("Finished conversion of w:{},h:{}", img.width(), img.height());
				// 		body = _body.into_inner().unwrap().into_inner().into();
				// 	}
				// }
				_ => {}
			}
		}
		let id_in = id.clone();
		{
			// Calculate hash
			let mut hasher = DefaultHasher::new();
			body.hash(&mut hasher);
			id = hasher.finish().to_quint();
		}
		if id_in != id {
			// // Means conversion changed the data
			// cache
			// 	.write()
			// 	.unwrap()
			// 	.media
			// 	.insert(id_in, crate::MediaEntry::Ref(id.clone()));
		}
		let path = path.join(&id);

		if !path.exists() {
			if let Err(err) = tokio::fs::write(path, body).await {
				return Err((StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", err)));
			}
		}
		// cache.write().unwrap().media.insert(
		// 	id.clone(),
		// 	crate::MediaEntry::Entry {
		// 		user: Some(user_claims.user),
		// 		_type: matcher_type,
		// 	},
		// );
	}

	Ok(Json(MediaPostResponse {
		id,
		_type: matcher_type,
	}))
}
