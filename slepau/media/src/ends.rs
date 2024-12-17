use std::ops::Bound::{Excluded, Included, Unbounded};

use auth::UserClaims;
use axum::{
	body::StreamBody,
	extract::{BodyStream, Path, Query},
	headers,
	http::header,
	response::{IntoResponse, Response},
	Extension, Json, TypedHeader,
};
use common::{
	socket::ResourceSender,
	utils::{DbError, LockedAtomic, CACHE_FOLDER},
	vreji::log_ip_user_id,
};
use futures::{future::join_all, join};
use hyper::StatusCode;

use log::info;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio_util::{bytes::Bytes, io::ReaderStream};

use media::{MatcherType, MEDIA_FOLDER};

use axum_client_ip::InsecureClientIp;
type ClientIp = InsecureClientIp;

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
	range: Option<TypedHeader<headers::Range>>,
	ip: ClientIp,
	Query(any): Query<Any>,
	Extension(db): Extension<LockedAtomic<DB>>,
	Extension(user_claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, Response> {
	let path;
	let mut version = Default::default();
	let mut wants_raw = false;
	match any {
		Any::Options(opts) => wants_raw = opts.raw == "true",
		Any::Version(_version) => version = _version,
	};
	let version_empty = json!(version).as_object().unwrap().is_empty();

	log_ip_user_id("media_get", ip.0, &user_claims.user, id.inner());

	if wants_raw {
		// Return original path
		path = VersionReference::to_path_in(id);
	} else if version_empty {
		// If default_version path exists, return that path,
		// or schedule a default_version task and return original path
		path = db.write().unwrap().default_version_path(id);
	} else {
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
		Err(err) => {
			return Err(
				(StatusCode::NOT_FOUND, format!("File not found: {}", err)).into_response(),
			)
		}
	};

	let mut buf = [0u8; 64];
	if let Ok(_buf_size) = file.read(&mut buf).await {
		file.rewind().await.unwrap(); // Reset the counter to start of file
		let _type = infer::get(&buf);

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

		let full_response = |file| {
			// // convert the `AsyncRead` into a `Stream`
			let stream = ReaderStream::new(file);
			// // convert the `Stream` into an `axum::body::HttpBody`
			let body = StreamBody::new(stream);

			Ok((StatusCode::OK, headers.clone(), body).into_response())
		};

		if let Some(TypedHeader(range)) = range {
			let file_size = file.metadata().await.unwrap().len();
			let byte_bounds = range
				.iter()
				.map(|(a, b)| {
					if let Included(b) = b {
						if let Unbounded = a {
							return (file_size - b, file_size - 1);
						}
					}
					(
						match a {
							Unbounded => 0,
							Included(v) => v,
							Excluded(v) => v + 1,
						},
						match b {
							Unbounded => file_size - 1,
							Included(v) => v,
							Excluded(v) => v - 1,
						},
					)
				})
				.collect::<Vec<_>>();
			// If bounds is same as whole file, return whole file
			if byte_bounds.is_empty()
				|| (byte_bounds[0].0 == 0 && byte_bounds[0].1 == file_size - 1)
			{
				return full_response(file);
			}
			for (i, (x1, x2)) in byte_bounds.iter().enumerate() {
				if i == byte_bounds.len() - 1 {
					break;
				}
				for (y1, y2) in byte_bounds[i + 1..].iter() {
					if x1 <= y2 || y1 <= x2 {
						return Ok(StatusCode::RANGE_NOT_SATISFIABLE.into_response());
					}
				}
			}

			let mut buffer = vec![0; file_size as usize];
			file.read_exact(&mut buffer).await.expect("buffer overflow");
			let mut body = Vec::<u8>::with_capacity(file_size as usize);
			for (i, byte) in buffer.iter().enumerate() {
				if byte_bounds.iter().any(|(a, b)| *a <= i as u64 && i as u64 <= *b) {
					body.push(*byte)
				}
			}

			Ok((StatusCode::PARTIAL_CONTENT, headers, Bytes::from(body)).into_response())
		} else {
			full_response(file)
		}
	} else {
		Err((StatusCode::NO_CONTENT, "Error reading file?".to_string()).into_response())
	}
}

pub async fn media_delete(
	Path(id): Path<MediaId>,
	ip: ClientIp,
	Extension(user_claims): Extension<UserClaims>,
	Extension(db): Extension<LockedAtomic<DB>>,
) -> Result<impl IntoResponse, DbError> {
	let media = Media::from(db.write().unwrap().del(id, &user_claims.user)?);

	let cache = std::path::Path::new(CACHE_FOLDER.as_str());
	let removes = media
		.versions
		.iter()
		.map(|version| {
			tokio::fs::remove_file(
				cache.join(VersionReference::from((media.id, version.0.clone())).filename_out()),
			)
		})
		.collect::<Vec<_>>();

	let removes = join_all(removes);

	let path = std::path::Path::new(MEDIA_FOLDER.as_str()).join(media.id.to_quint());
	let original = tokio::fs::remove_file(path);

	let (_removes, original) = join!(removes, original);

	original.map_err(|_| DbError::from("File not found"))?;

	log_ip_user_id("media_del", ip.0, &user_claims.user, id.inner());

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
	ip: ClientIp,
	mut body: BodyStream,
) -> Result<impl IntoResponse, impl IntoResponse> {
	if user_claims.user == "public" && !db.read().unwrap().allow_public_post {
		// body.count().await;
		return Err((
			StatusCode::FORBIDDEN,
			"Public isn't allowed to upload.".to_string(),
		));
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
		log_ip_user_id("media_post_error", ip.0, &user_claims.user, 0);
		log::error!(
			"{} reached his limit of {} MB",
			user_claims.user,
			user_claims.media_limit / MB
		);
		return Err((
			StatusCode::FORBIDDEN,
			format!(
				"You've reached your limit of {} MB.
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
				log_ip_user_id("media_post_error", ip.0, &user_claims.user, 1);
				return Err((
					StatusCode::PAYLOAD_TOO_LARGE,
					format!("Body > {} MB", MAX_ALLOWED_MEDIA_SIZE / MB),
				));
			}
			file.write_buf(&mut chunk).await.unwrap();
		}
		// info!("write total {}", size);
		file.sync_all().await.unwrap();
	}

	let media = db
		.write()
		.unwrap()
		.add((id, &path).into(), user_claims.user.clone());
	let media = media.read().unwrap().clone();

	// Notify
	tx_resource
		.send(("media", [user_claims.user.to_owned()].into()).into())
		.ok();

	log_ip_user_id("media_post", ip.0, &user_claims.user, id.inner());

	Ok(Json(media))
}

pub async fn stats(
	Extension(db): Extension<LockedAtomic<DB>>,
	// Extension(user_claims): Extension<UserClaims>,
) -> impl IntoResponse {
	Json(DBStats::from(&*db.read().unwrap()))
}
