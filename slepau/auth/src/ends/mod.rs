use auth::{
	validate::{KP, KPR},
	UserClaims,
};
use axum::{
	extract::{Extension, Query},
	headers,
	http::header,
	http::HeaderMap,
	response::IntoResponse,
	Json, TypedHeader,
};
use common::utils::{get_secs, hostname_normalize, DbError, LockedAtomic, SECS_IN_DAY, SECURE, WEB_DIST};
use hyper::StatusCode;
use lazy_static::lazy_static;
use log::{error, info};
use pasetors::{claims::Claims, local, public};
use serde::Deserialize;
use std::path::PathBuf;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub mod admin;

use crate::db::DBAuth;

pub async fn home_service(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, impl IntoResponse> {
	let host = hostname_normalize(host.hostname());

	fn get_src() -> Option<String> {
		std::fs::read_to_string(PathBuf::from(WEB_DIST.as_str()).join("home.html")).ok()
	}

	let src;
	// If we're debugging, get every time
	if cfg!(debug_assertions) {
		src = get_src();
	} else {
		lazy_static! {
			static ref SRC: Option<String> = get_src();
		}
		src = SRC.to_owned();
	}

	src
		.as_ref()
		.map(|src| {
			(
				[(header::CONTENT_TYPE, "text/html")],
				src
					.replace("_HOST_", serde_json::to_string(host).unwrap().as_str())
					.replace("_USER_", serde_json::to_string(&claims).unwrap().as_str()),
			)
		})
		.ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}
/// Populates index.html with the needed request data
pub async fn login_service(
	headers: HeaderMap,
	// Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, impl IntoResponse> {
	fn get_src() -> Option<String> {
		std::fs::read_to_string(PathBuf::from(WEB_DIST.as_str()).join("login.html")).ok()
	}
	let referer = headers.get("Referer").map(|v| v.to_str().unwrap()).unwrap_or_default();

	let src;
	// If we're debugging, get every time
	if cfg!(debug_assertions) {
		src = get_src();
	} else {
		lazy_static! {
			static ref SRC: Option<String> = get_src();
		}
		src = SRC.to_owned();
	}

	src
		.as_ref()
		.map(|src| {
			(
				[(header::CONTENT_TYPE, "text/html")],
				src
					.replace("_REFERER_", serde_json::to_string(referer).unwrap().as_str())
					.replace("_USER_", serde_json::to_string(&claims).unwrap().as_str()),
			)
		})
		.ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct LoginQuery {
	admin: bool,
}

pub async fn login(
	TypedHeader(host): TypedHeader<headers::Host>,
	Query(query): Query<LoginQuery>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Json((user, pass)): Json<(String, String)>,
) -> Result<impl IntoResponse, DbError> {
	let db = db.write().unwrap();

	let (host, mut site_id) = db.host_to_site_id(host.hostname());
	// Throw error if no site found and user doesn't wanna login as admin.
	// To prevent an inadvert login as admin.
	if query.admin {
		site_id = None;
	} else if site_id.is_none() {
		return Err(DbError::InvalidSite("No site setup yet for this host. Contact admin."));
	}

	db.login(&user, &pass, site_id)
		.map(|(user, is_admin, is_super, max_age)| {
			// Create token
			let mut claims = Claims::new().unwrap();
			// Set Issuer
			claims.issuer("slepau:auth").unwrap();
			// Set Audience
			claims.audience(&host).unwrap();
			//
			user
				.claims
				.into_iter()
				.filter(|(k, _)| k != "admin" && k != "super")
				.for_each(|(k, v)| {
					claims.add_additional(&k, v).ok();
				});

			claims.add_additional("user", user.user).unwrap();

			if is_admin {
				claims.add_additional("admin", is_admin).unwrap();
			}
			if is_super {
				claims.add_additional("super", is_super).unwrap();
			}

			let iat = OffsetDateTime::from_unix_timestamp(get_secs().try_into().unwrap())
				.unwrap()
				.format(&Rfc3339)
				.unwrap();
			let exp = get_secs() + max_age as u64; // 7 days
			let exp = OffsetDateTime::from_unix_timestamp(exp.try_into().unwrap())
				.unwrap()
				.format(&Rfc3339)
				.unwrap();

			claims.not_before(&iat).unwrap();
			claims.issued_at(&iat).unwrap();
			claims.expiration(&exp).unwrap();

			// Generate the keys and sign the claims.
			// let pub_token = private::sign(&KP.secret, &claims, None, None).unwrap();
			let pub_token = local::encrypt(&KPR, &claims, None, None).unwrap();

			[(
				header::SET_COOKIE,
				format!(
					"auth={pub_token}; Domain={}; Path=/; SameSite=Strict; Max-Age={max_age}; HttpOnly; {}",
					&host,
					if *SECURE { " Secure;" } else { "" }
				),
			)]
		})
		.map_err(|err| {
			error!("Failed login for '{}' with pass '{}': {:?}.", &user, &pass, &err);
			err
		})
}
pub async fn register(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Json((user, pass)): Json<(String, String)>,
) -> Result<impl IntoResponse, DbError> {
	let mut db = db.write().unwrap();

	if db.admins.is_empty() {
		db.new_admin(&user, &pass)?;
		info!("Super admin created '{}'.", &user);
		return Ok("Super admin created.");
	}

	let (_, site_id) = db.host_to_site_id(host.hostname());
	let site_id = site_id.ok_or(DbError::InvalidSite("A site hasn't been setup yet."))?;

	db.new_user(&user, &pass, site_id)
		.map(|_| {
			info!("User created '{}'.", &user);
			"User created."
		})
		.map_err(|err| {
			error!("Failed register for '{}' with pass '{}': {:?}.", &user, &pass, &err);
			err
		})
}

pub async fn reset(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Json((user, old_pass, pass)): Json<(String, String, String)>,
) -> Result<impl IntoResponse, DbError> {
	let mut db = db.write().unwrap();

	let (_, site_id) = db.host_to_site_id(host.hostname());
	let site_id = site_id.ok_or(DbError::NotFound)?;

	db.reset(&user, &pass, &old_pass, Some(site_id))
		.map(|_| {
			info!("User password reset '{user}'.");
			"User pass reset."
		})
		.map_err(|err| {
			error!("Failed password reset for '{user}' with old_pass '{old_pass}': {err:?}.");
			err
		})
}

pub async fn user(
	Extension(_db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
) -> impl IntoResponse {
	Json(user_claims)
}
pub async fn logout(
	TypedHeader(host): TypedHeader<headers::Host>,
	headers: HeaderMap,
) -> impl IntoResponse {
	let host_full = host.hostname();
	let host = hostname_normalize(host.hostname());

	let referer = headers.get("Referer").map(|v| v.to_str().unwrap()).unwrap_or_default();
	(
		StatusCode::FOUND,
		[
			(
				header::SET_COOKIE,
				format!(
					"auth=; Domain={}; Path=/; SameSite=Strict; Max-Age=0; HttpOnly; {}",
					&host,
					if *SECURE { " Secure;" } else { "" }
				),
			),
			// (
			// 	header::SET_COOKIE,
			// 	format!(
			// 		"auth=; Domain={}; Path=/; SameSite=Strict; Max-Age=0; HttpOnly; {}",
			// 		host_full,
			// 		if *SECURE { " Secure;" } else { "" }
			// 	),
			// ),
			(header::LOCATION, format!("{referer}") ),
		],
	)
}
