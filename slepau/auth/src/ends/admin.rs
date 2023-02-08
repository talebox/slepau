use auth::{validate::KP, UserClaims};
use axum::{
	extract::{Extension, Path},
	headers,
	http::header,
	response::IntoResponse,
	Json, TypedHeader,
};
use common::utils::{get_secs, DbError, LockedAtomic, SECS_IN_DAY, SECURE, URL};
use hyper::StatusCode;
use log::{error, info};
use pasetors::{claims::Claims, public};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
	db::{site::SiteView, DBAuth},
	user::UserView,
};

// GET
pub async fn get_users(
	Path(admin): Path<String>,
	Path(site_id): Path<String>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
) -> Result<impl IntoResponse, DbError> {
	
}
pub async fn get_sites(
	Path(admin): Path<String>,
	Path(site_id): Path<String>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
) -> Result<impl IntoResponse, DbError> {
}
pub async fn get_admins(Extension(db): Extension<LockedAtomic<DBAuth>>) -> Result<impl IntoResponse, DbError> {
	
}

// PUT
pub async fn put_user(
	Path(admin): Path<String>,
	Path(site_id): Path<String>,
	Path(user_id): Path<String>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Json(user_in): Json<UserView>,
) -> Result<impl IntoResponse, DbError> {
}
pub async fn put_admin(
	Path(user_id): Path<String>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Json(user_in): Json<UserView>,
) -> Result<impl IntoResponse, DbError> {
}
pub async fn put_site(
	Path(admin): Path<String>,
	Path(site_id): Path<String>,
	Path(user_id): Path<String>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Json(user_in): Json<SiteView>,
) -> Result<impl IntoResponse, DbError> {
}

// DEL
pub async fn del_user(
	Path(admin): Path<String>,
	Path(site_id): Path<String>,
	Path(user_id): Path<String>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
) -> Result<impl IntoResponse, DbError> {
}

pub async fn del_site(
	Path(admin): Path<String>,
	Path(site_id): Path<String>,
	Path(user_id): Path<String>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
) -> Result<impl IntoResponse, DbError> {
}
pub async fn del_admin(
	Path(user_id): Path<String>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
) -> Result<impl IntoResponse, DbError> {
}
