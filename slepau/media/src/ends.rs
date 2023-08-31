use auth::UserClaims;
use axum::{
	body::{StreamBody},
	extract::{BodyStream, Path, Query},
	http::header,
	response::{IntoResponse, Response},
	Extension, Json,
};
use common::{
	socket::ResourceSender,
	utils::{DbError, LockedAtomic, CACHE_FOLDER},
};
use futures::{future::join_all, join};
use hyper::StatusCode;

use log::info;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio_util::io::ReaderStream;

use media::{MatcherType, MEDIA_FOLDER};

use crate::db::{
	task::Task,
	version::{Version, VersionReference},
	DBStats, Media, MediaId, DB,
};

#[derive(Deserialize)]
pub struct Options {
	raw: String,
}
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Any {
	Options(Options),
	Version(Version),
}

pub async fn media_get(
	Path(id): Path<MediaId>,
	Query(any): Query<Any>,
	Extension(db): Extension<LockedAtomic<DB>>,
	// Extension(tx_task): Extension<tokio::sync::mpsc::Sender<Task>>,
) -> Result<impl IntoResponse, Response> {
	let path;
	let mut version = Default::default();
	let mut wants_raw = false;
	match any {
		Any::Options(opts) => wants_raw = opts.raw == "true",
		Any::Version(_version) => version = _version,
	};
	let version_empty = json!(version).as_object().unwrap().len() == 0;

	if wants_raw {
		// if cfg!(debug_assertions) {
		// 	info!("User wants raw for {id}");
		// }
		path = VersionReference::to_path_in(id);
	} else if version_empty {
		// if cfg!(debug_assertions) {
		// 	info!("User wants default for {id}");
		// }
		path = db.write().unwrap().default_version_path(id);
		// if cfg!(debug_assertions) {
		// 	info!("Deafult is {path:?}");
		// }
	// Schedule the task and don't wait for it
	// let task = Task::from((0, version_ref.clone()));
	// tx_task.send(task).await.unwrap();
	} else {
		// if cfg!(debug_assertions) {
		// 	info!("User wants {id} version {}", VersionString::from(&version));
		// }
		// Wait for task, someone wants something specific
		let _ref: VersionReference = (id, (&version).into()).into();
		let _path = db.read().unwrap().version_path(_ref);
		match _path {
			Ok(_path) => {
				path = _path;
			}
			Err(_ref) => {
				let _ref = _ref.map_err(|e| e.into_response())?;
				let (sender, receiver) = tokio::sync::oneshot::channel();
				let task = Task::from((10, _ref.clone(), sender));
				db.write().unwrap().queue(task);
				receiver
					.await
					.map_err(|_| "Receiver error".into())
					.flatten()
					.map_err(|e| e.into_response())?;
				path = db.read().unwrap().version_path(_ref).unwrap();
			}
		}
	}

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
	Extension(user_claims): Extension<UserClaims>,
	Extension(db): Extension<LockedAtomic<DB>>,
) -> Result<impl IntoResponse, DbError> {
	let media = Media::from(db.write().unwrap().del(id, &user_claims.user)?);

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

	let (_removes, original) = join!(removes, original);

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

	if let Some(v) = media_patch.name {
		media.name = v
	}

	Ok(Json(media.clone()))
}

#[derive(Serialize)]
pub struct MediaPostResponse {
	id: MediaId,

	#[serde(with = "MatcherType", rename = "type")]
	_type: infer::MatcherType,
}

use futures::StreamExt;

/// Body arrives, we write to disk and return id
///
/// Conversion happens after.
pub async fn media_post(
	Extension(db): Extension<LockedAtomic<DB>>,
	Extension(tx_resource): Extension<ResourceSender>,
	Extension(user_claims): Extension<UserClaims>,
	mut body: BodyStream,
) -> Result<impl IntoResponse, impl IntoResponse> {
	if user_claims.user == "public" && !db.read().unwrap().allow_public_post {
		// body.count().await;
		return Err((StatusCode::FORBIDDEN, format!("Public isn't allowed to upload.")));
	}

	let path = std::path::Path::new(MEDIA_FOLDER.as_str());
	if !path.exists() {
		tokio::fs::create_dir(&path).await.unwrap();
		info!("Created media folder at '{}'.", path.to_string_lossy());
	}

	const MB: u64 = 1024 * 1024;
	const MAX_ALLOWED_MEDIA_SIZE: u64 = 100 * MB;

	let stats = db.read().unwrap().user_stats(&user_claims.user);
	// info!(
	// 	"Stats for {} are {}",
	// 	&user_claims.user,
	// 	serde_json::to_string(&stats).unwrap()
	// );

	if user_claims.media_limit > 0 && stats.size >= user_claims.media_limit {
		// body.count().await;
		log::error!(
			"{} reached his limit of {}MB",
			user_claims.user,
			user_claims.media_limit / MB
		);
		return Err((
			StatusCode::FORBIDDEN,
			format!(
				"You've reached your limit of {}MB.
				\nContact your admin for a limit upgrade or optimize some of your media.",
				user_claims.media_limit / MB
			),
		));
	}

	let id = db.read().unwrap().new_id();
	let path = path.join(id.to_quint());
	{
		let mut file = tokio::fs::File::create(path.clone()).await.unwrap();

		let mut size: u64 = 0;
		while let Some(Ok(mut chunk)) = body.next().await {
			// info!("write chunk {}", chunk.len());
			size += chunk.len() as u64;
			if size >= MAX_ALLOWED_MEDIA_SIZE {
				return Err((
					StatusCode::PAYLOAD_TOO_LARGE,
					format!("Body > {}MB", MAX_ALLOWED_MEDIA_SIZE / MB),
				));
			}
			file.write_buf(&mut chunk).await.unwrap();
		}
		// info!("write total {}", size);
		file.sync_all().await.unwrap();
	}

	let media = db.write().unwrap().add((id, &path).into(), user_claims.user.clone());
	let media = media.read().unwrap().clone();

	// Notify
	tx_resource.send(("media", [user_claims.user].into()).into()).ok();

	Ok(Json(media))
}

pub async fn stats(
	Extension(db): Extension<LockedAtomic<DB>>,
	// Extension(user_claims): Extension<UserClaims>,
) -> impl IntoResponse {
	Json(DBStats::from(&*db.read().unwrap()))
}
