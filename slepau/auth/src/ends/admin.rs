use auth::UserClaims;
use axum::{
	extract::{Extension, Path, Query},
	response::IntoResponse,
	Json,
};
use common::utils::{DbError, LockedAtomic};
use log::info;

use crate::{
	db::{
		get::Filter,
		site::{AdminSet, SiteId, SiteSet},
		DBAuth,
	},
	user::UserSet,
};

// NEW
pub async fn post_admin(
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
	Json((user, pass)): Json<(String, String)>,
) -> Result<impl IntoResponse, DbError> {
	db.write().unwrap().new_admin(&user, &pass)?;
	info!("New admin {} created by {}", user, user_claims.user);
	Ok(())
}
pub async fn post_site(
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, DbError> {
	let id = db.write().unwrap().new_site(&user_claims.user)?;
	info!("New site created by {}", user_claims.user);
	Ok(Json(id))
}
pub async fn post_user(
	Path(site_id): Path<SiteId>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
	Json((user, pass)): Json<(String, String)>,
) -> Result<impl IntoResponse, DbError> {
	db.write().unwrap().new_user(&user, &pass, site_id)?;
	info!("New user {} created by {}", user, user_claims.user);
	Ok(())
}

// GET
pub async fn get_admins(
	Query(filter): Query<Filter>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, DbError> {
	Ok(Json(db.read().unwrap().get_admins(&user_claims.user, filter)?))
}
pub async fn get_sites(
	Query(filter): Query<Filter>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, DbError> {
	Ok(Json(db.read().unwrap().get_sites(&user_claims.user, filter)?))
}
pub async fn get_users(
	Query(filter): Query<Filter>,
	Path(site_id): Path<SiteId>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, DbError> {
	Ok(Json(db.read().unwrap().get_users(
		&user_claims.user,
		site_id,
		filter,
	)?))
}

// PUT
pub async fn put_admin(
	Path(user_id): Path<String>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
	Json(v): Json<AdminSet>,
) -> Result<impl IntoResponse, DbError> {
	db.write().unwrap().mod_admin(&user_claims.user, &user_id, v)
}
pub async fn put_site(
	Path(site_id): Path<SiteId>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
	Json(v): Json<SiteSet>,
) -> Result<impl IntoResponse, DbError> {
	db.write().unwrap().mod_site(&user_claims.user, site_id, v)
}
pub async fn put_user(
	Path((site_id, user_id)): Path<(SiteId, String)>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
	Json(v): Json<UserSet>,
) -> Result<impl IntoResponse, DbError> {
	db.write().unwrap().mod_user(&user_claims.user, site_id, &user_id, v)
}

// DEL
pub async fn del_admin(
	Path(user_id): Path<String>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, DbError> {
	db.write().unwrap().del_admin(&user_claims.user, &user_id)
}
pub async fn del_site(
	Path(site_id): Path<SiteId>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, DbError> {
	db.write().unwrap().del_site(&user_claims.user, site_id)
}
pub async fn del_user(
	Path((site_id, user_id)): Path<(SiteId, String)>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, DbError> {
	db.write().unwrap().del_user(&user_claims.user, site_id, &user_id)
}
