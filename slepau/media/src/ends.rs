use auth::UserClaims;
use axum::{
	body::{Bytes, HttpBody, StreamBody},
	extract::{Path, Query, RawBody},
	http::header,
	response::{IntoResponse, Response},
	Extension, Json,
};
use common::{
	socket::{ResourceMessage, ResourceSender, SocketMessage},
	utils::{DbError, LockedAtomic, CACHE_FOLDER, WEB_DIST},
};
use futures::{future::join_all, join};
use hyper::{body::to_bytes, StatusCode};

use log::info;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use lazy_static::lazy_static;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use media::{MatcherType, MEDIA_FOLDER};

use crate::db::{
	task::Task,
	version::{Version, VersionReference, Max},
	DBStats, Media, MediaId, DB,
};

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
				home.replace("_USER_", serde_json::to_string(&claims).unwrap().as_str()),
			)
		})
		.ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn media_get(
	Path(id): Path<MediaId>,
	Query(mut version): Query<Version>,
	Extension(db): Extension<LockedAtomic<DB>>,
	Extension(tx_task): Extension<tokio::sync::mpsc::Sender<Task>>,
) -> Result<impl IntoResponse, Response> {
	// Try getting the version, else prioritize the version
	
	version.max = version.max.or_else(|| Some(Max::Absolute(Some(300), Some(300))));

	let version_ref = (id, (&version).into()).into();
	let mut path = db.read().unwrap().version_path(&version_ref);
	if path.is_none() {
		info!("Generating {version_ref:?}");
		// Try getting the path by scheduling a task
		let (sender, receiver) = tokio::sync::oneshot::channel();
		let task = Task::from((10, version_ref.clone(), sender));

		tx_task.send(task).await.unwrap();
		receiver.await.unwrap().map_err(|e| e.into_response())?;
		path = db.read().unwrap().version_path(&version_ref);
	}
	let path = path.ok_or((StatusCode::NOT_FOUND, format!("Media can't be resolved")).into_response())?;

	let mut file = match tokio::fs::File::open(&path).await {
		Ok(file) => file,
		Err(err) => return Err((StatusCode::NOT_FOUND, format!("File not found: {}", err)).into_response()),
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
		Err((StatusCode::NO_CONTENT, "Error reading file?".to_string()).into_response())
	}
}

pub async fn media_delete(
	Path(id): Path<MediaId>,
	Extension(db): Extension<LockedAtomic<DB>>,
) -> Result<impl IntoResponse, DbError> {
	let media = Media::from(db.write().unwrap().del(id)?);

	let cache = std::path::Path::new(CACHE_FOLDER.as_str());
	let removes = media
		.versions
		.iter()
		.map(|version| {
			tokio::fs::remove_file(cache.join(VersionReference::from((media.id, version.0.clone())).filename_out()))
		})
		.collect::<Vec<_>>();

	let removes = join_all(removes);

	let path = std::path::Path::new(MEDIA_FOLDER.as_str()).join(media.id.to_quint());
	let original = tokio::fs::remove_file(path);

	let (removes, original) = join!(removes, original);

	original.map_err(|_| DbError::from("File not found"))?;

	Ok(Json(media))
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct MediaPatch {
	name: Option<String>,
}

pub async fn media_patch(
	Path(id): Path<MediaId>,
	Extension(db): Extension<LockedAtomic<DB>>,
	Json(media_patch): Json<MediaPatch>,
) -> Result<impl IntoResponse, DbError> {
	let media = db.read().unwrap().get(id).ok_or(DbError::NotFound)?;
	let mut media = media.write().unwrap();

	if let Some(v) = media_patch.name {media.name = v}

	Ok(Json(media.clone()))
}

#[derive(Serialize)]
pub struct MediaPostResponse {
	id: MediaId,

	#[serde(with = "MatcherType", rename = "type")]
	_type: infer::MatcherType,
}

/// Body arrives, we write to disk and return id
///
/// Conversion happens after.
pub async fn media_post(
	Extension(db): Extension<LockedAtomic<DB>>,
	Extension(tx_resource): Extension<ResourceSender>,
	Extension(user_claims): Extension<UserClaims>,
	body: RawBody,
) -> Result<impl IntoResponse, impl IntoResponse> {
	let path = std::path::Path::new(MEDIA_FOLDER.as_str());
	if !path.exists() {
		tokio::fs::create_dir(&path).await.unwrap();
		info!("Created media folder at '{}'.", path.to_string_lossy());
	}

	const MAX_ALLOWED_RESPONSE_SIZE: u64 = 1024 * 1024 * 100; // 100mb;

	// Check if body isn't too big
	if body.0.size_hint().upper().map(|v| v < MAX_ALLOWED_RESPONSE_SIZE) != Some(true) {
		return Err((StatusCode::PAYLOAD_TOO_LARGE, format!("Body > 100mb")));
	}

	// Get the body
	let body = to_bytes(body.0).await.unwrap();

	let id = db.read().unwrap().new_id();

	// Write to disk
	let path = path.join(id.to_quint());
	tokio::fs::write(path, &body).await.unwrap();
	let media = db.write().unwrap().add((id, &body.to_vec()).into(), user_claims.user);
	let media = media.read().unwrap().clone();

	// Notify
	tx_resource.send("media".into()).ok();

	Ok(Json(media))
}

pub async fn stats(
	Extension(db): Extension<LockedAtomic<DB>>,
	// Extension(user_claims): Extension<UserClaims>,
) -> impl IntoResponse {
	Json(DBStats::from(&*db.read().unwrap()))
}
