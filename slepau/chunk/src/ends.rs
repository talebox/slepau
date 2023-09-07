use auth::UserClaims;
use axum::{
	extract::{Extension, Path},
	response::IntoResponse,
	Json, TypedHeader,
};
use common::{
	socket::{ResourceMessage, ResourceSender},
	utils::{DbError, LockedAtomic}, vreji::log_ip_user_id,
};
use headers::ContentType;

use axum_client_ip::InsecureClientIp;
type ClientIp = InsecureClientIp;

use log::{info, trace};
use serde::Deserialize;
use std::collections::HashSet;

use crate::{
	db::{
		chunk::{Chunk, ChunkId},
		dbchunk::DBChunk,
		view::ChunkView,
		DB,
	},
	format::value_to_html,
};

// pub async fn chunks_get(
// 	Extension(db): Extension<LockedAtomic<DB>>,
// 	Extension(user_claims): Extension<UserClaims>,
// ) -> Result<impl IntoResponse, DbError> {
// 	info!("User is {}.", &user_claims.user);

// 	let mut chunks: Vec<Chunk> = db
// 		.write()
// 		.unwrap()
// 		.get_chunks(&user_claims.user)
// 		.into_iter()
// 		.map(|v| v.read().unwrap().chunk().clone())
// 		.collect();
// 	chunks.sort_by_key(|v| -(v.modified as i64));

// 	trace!("GET /chunks len {}", chunks.len());

// 	Ok(Json(chunks))
// }
pub async fn chunks_get_id(
	Path(id): Path<ChunkId>,
	Extension(db): Extension<LockedAtomic<DB>>,
	Extension(user_claims): Extension<UserClaims>,
	ip: ClientIp,
) -> Result<impl IntoResponse, DbError> {
	if let Some(chunk) = if user_claims._super {
		db.read().unwrap().get_chunk_(id)
	} else {
		db.read().unwrap().get_chunk(id, &user_claims.user)
	} {
		log_ip_user_id("chunk_get_id", ip.0, &user_claims.user, id.inner().into());
		Ok(Json(chunk.read().unwrap().chunk().clone()))
	} else {
		Err(DbError::NotFound)
	}
}
pub async fn page_get_id(
	Path(id): Path<ChunkId>,
	Extension(db): Extension<LockedAtomic<DB>>,
	Extension(user_claims): Extension<UserClaims>,
	ip: ClientIp,
) -> Result<impl IntoResponse, DbError> {
	if let Some(chunk) = if user_claims._super {
		db.read().unwrap().get_chunk_(id)
	} else {
		db.read().unwrap().get_chunk(id, &user_claims.user)
	} {
		let mut title: String = "Page".into();
		let html;
		{
			let lock = chunk.read().unwrap();
			if let Some(v) = lock.get_prop::<String>("title") {
				title = v
			};
			html = value_to_html(&lock.chunk().value);
		}
		let page = include_str!(env!("CHUNK_PAGE_PATH"));
		let page = page.replace("PAGE_TITLE", &title);
		let page = page.replace("PAGE_BODY", &html);
		log_ip_user_id("chunk_get_page", ip.0, &user_claims.user, id.inner().into());
		Ok((TypedHeader(ContentType::html()), page))
	} else {
		Err(DbError::NotFound)
	}
}

#[derive(Debug, Deserialize, Default)]
pub struct ChunkIn {
	id: Option<ChunkId>,
	value: String,
}

pub async fn chunks_put(
	Extension(db): Extension<LockedAtomic<DB>>,
	Extension(user_claims): Extension<UserClaims>,
	Extension(tx_r): Extension<ResourceSender>,
	ip: ClientIp,
	Json(body): Json<ChunkIn>,
) -> Result<impl IntoResponse, DbError> {
	let db_chunk = DBChunk::from((body.id, body.value.as_str(), user_claims.user.as_str()));
	let users = db_chunk.access_users();
	let id = db_chunk.chunk().id;
	let users_to_notify = db.write().unwrap().set_chunk(db_chunk, &user_claims.user)?;

	// Notifies users for which access has changed
	// They should request an update of their active view that uses chunks
	// upon this request
	tx_r.send(ResourceMessage::from(("chunks", users_to_notify))).unwrap();
	log_ip_user_id("chunk_put", ip.0, &user_claims.user, id.inner().into());
	// Notifies users which already have access, of the note's new content
	//
	// Only do so if modifying a chunk, because a new one won't have an id.
	// Because the user that created it will ask for them anyway almost immediately
	// since we will have told them that they have to update their view up there ^
	if let Some(id) = body.id {
		let chunk = ChunkView::from((
			db.read().unwrap().get_chunk(id, &user_claims.user).unwrap(),
			user_claims.user.as_str(),
		));

		tx_r
			.send(ResourceMessage::from((
				format!("chunks/{}", id).as_str(),
				users,
				&chunk,
			)))
			.unwrap();
	}

	Ok(())
}

pub async fn chunks_del(
	Extension(db): Extension<LockedAtomic<DB>>,
	Extension(user_claims): Extension<UserClaims>,
	Extension(tx_r): Extension<ResourceSender>,
	ip: ClientIp,
	Json(input): Json<HashSet<ChunkId>>,
) -> Result<impl IntoResponse, DbError> {
	let users_to_notify = db.write().unwrap().del_chunk(input.to_owned(), &user_claims.user)?;

	input.into_iter().for_each(|id| {
		log_ip_user_id("chunk_del", ip.0, &user_claims.user, id.inner().into());
	});

	tx_r.send(ResourceMessage::from(("chunks", users_to_notify))).unwrap();

	Ok(())
}
