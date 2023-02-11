use auth::{
	validate::{KP, KPR},
	UserClaims,
};
use axum::{extract::Extension, headers, http::header, response::IntoResponse, Json, TypedHeader};
use common::utils::{get_secs, DbError, LockedAtomic, SECS_IN_DAY, SECURE, WEB_DIST};
use hyper::StatusCode;
use lazy_static::lazy_static;
use log::{error, info};
use pasetors::{claims::Claims, local, public};
use std::path::PathBuf;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub mod admin;

use crate::db::DBAuth;

pub async fn home_service(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, impl IntoResponse> {
	let (host, site_id) = db.read().unwrap().host_to_site_id(host.hostname());

	fn get_home() -> Option<String> {
		std::fs::read_to_string(PathBuf::from(WEB_DIST.as_str()).join("home.html")).ok()
	}

	let home;
	// If we're debugging, get home every time
	if cfg!(debug_assertions) {
		home = get_home();
	} else {
		lazy_static! {
			static ref HOME: Option<String> = get_home();
		}
		home = HOME.to_owned();
	}

	home
		.as_ref()
		.map(|home| {
			(
				[(header::CONTENT_TYPE, "text/html")],
				home
					.replace("_HOST_", &host)
					.replace("_USER_", serde_json::to_string(&claims).unwrap().as_str()),
			)
		})
		.ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn login(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Json((user, pass)): Json<(String, String)>,
) -> Result<impl IntoResponse, DbError> {
	let db = db.write().unwrap();

	let (host, site_id) = db.host_to_site_id(host.hostname());

	db.login(&user, &pass, site_id)
		.map(|(user, is_admin, is_super)| {
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
			let exp = get_secs() + SECS_IN_DAY * 7; // 7 days
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
					"auth={pub_token}; SameSite=Strict; Max-Age={}; Path=/; HttpOnly;{}",
					60/*sec*/*60/*min*/*24/*hr*/*7, /*days = a week in seconds*/
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
		return Ok("Super admin created");
	}

	let (_, site_id) = db.host_to_site_id(host.hostname());
	let site_id = site_id.ok_or(DbError::InvalidSite("A site hasn't been setup yet."))?;

	db.new_user(&user, &pass, site_id)
		.map(|_| {
			info!("User created '{}'.", &user);
			"User created"
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
pub async fn logout() -> impl IntoResponse {
	[(
		header::SET_COOKIE,
		format!(
			"auth=; SameSite=Strict; Max-Age={}; Path=/;{}",
			60/*sec*/*60/*min*/*24/*hr*/*7, /*days*/
			/*= a week in seconds*/
			if *SECURE { " Secure;" } else { "" }
		),
	)]
}
