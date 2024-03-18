use auth::{validate::KPR, UserClaims};
use axum::{
	extract::{Extension, Path, Query},
	headers,
	http::header,
	http::HeaderMap,
	response::IntoResponse,
	Json, TypedHeader,
};
use common::{
	utils::{get_secs, hostname_normalize, DbError, LockedAtomic, SECURE},
	vreji::log_ip_user,
};
use hyper::StatusCode;

use log::{error, info};
use pasetors::{claims::Claims, local};
use serde::Deserialize;
use serde_json::Value;

use axum_client_ip::InsecureClientIp;
type ClientIp = InsecureClientIp;

use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub mod admin;

use crate::db::{site::SiteSet, DBAuth};

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct LoginQuery {
	admin: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct AuthBody {
	user: String,
	pass: String,
	old_pass: Option<String>,
}

pub async fn login(
	TypedHeader(host): TypedHeader<headers::Host>,
	Query(query): Query<LoginQuery>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	ip: ClientIp,
	Json(body): Json<AuthBody>,
) -> Result<impl IntoResponse, DbError> {
	let db = db.write().unwrap();
	let user = body.user;
	let pass = body.pass;

	let (host, mut site_id) = db.host_to_site_id(host.hostname());
	// Throw error if no site found and user doesn't wanna login as admin.
	// To prevent an inadvert login as admin.
	if query.admin {
		site_id = None;
	} else if site_id.is_none() {
		return Err(DbError::InvalidSite("No site setup yet for this host. Contact admin."));
	}

	db.login(&user, &pass, site_id)
		.map(|(user, site, is_admin, is_super, max_age)| {
			// Create token
			let mut claims = Claims::new().unwrap();
			// Set Issuer
			claims.issuer("talebox").unwrap();
			// Set Audience
			claims.audience(&host).unwrap();

			// Add site claims except 'admin' and 'super'
			if let Some(site) = site {
				site
					.claims
					.into_iter()
					.filter(|(k, _)| k != "admin" && k != "super")
					.for_each(|(k, v)| {
						claims.add_additional(&k, v).ok();
					});
			}

			// Add user claims except 'admin' and 'super'
			user
				.claims
				.into_iter()
				.filter(|(k, _)| k != "admin" && k != "super")
				.for_each(|(k, v)| {
					claims.add_additional(&k, v).ok();
				});

			claims.add_additional("user", user.user.clone()).unwrap();

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

			log_ip_user("auth_login", ip.0, &user.user);
			// let user_claims = UserClaims::from(&claims);
			// (
			[(
				header::SET_COOKIE,
				format!(
					"auth={pub_token}; Domain={host}; Path=/; SameSite=Lax; Max-Age={max_age}; HttpOnly; {}",
					if *SECURE { " Secure;" } else { "" }
				),
			)]
			// )
		})
		.map_err(|err| {
			error!("Failed login for '{}' with pass '{}': {:?}.", &user, &pass, &err);
			log_ip_user("auth_login_error", ip.0, &user);
			err
		})
}

pub async fn register(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	ip: ClientIp,
	Json(body): Json<AuthBody>,
) -> Result<impl IntoResponse, DbError> {
	let mut db = db.write().unwrap();
	let user = body.user;
	let pass = body.pass;

	if db.admins.is_empty() {
		db.new_admin(&user, &pass)?;
		let site_id = db.new_site(&user)?;
		db.mod_site(
			&user,
			site_id,
			SiteSet {
				name: "Default Site".into(),
				hosts: vec!["any".into()],
				max_age: 60 * 60 * 24,
				claims: Default::default(),
			},
		)?;
		info!("Super admin created '{user}' + New Default site '{site_id}'.");
		return Ok("Super admin created + New Default site.");
	}

	let (_, site_id) = db.host_to_site_id(host.hostname());
	let site_id = site_id.ok_or(DbError::InvalidSite("A site hasn't been setup yet."))?;

	db.new_user(&user, &pass, site_id)
		.map(|_| {
			info!("User created '{}'.", &user);
			log_ip_user("auth_register", ip.0, &user);
			"User created."
		})
		.map_err(|err| {
			error!("Failed register for '{}' with pass '{}': {:?}.", &user, &pass, &err);
			log_ip_user("auth_register_error", ip.0, &user);
			err
		})
}

pub async fn reset(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(user_claims): Extension<UserClaims>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	ip: ClientIp,
	Json(body): Json<AuthBody>,
) -> Result<impl IntoResponse, DbError> {
	let mut db = db.write().unwrap();
	let user = body.user;
	let pass = body.pass;
	let old_pass = body.old_pass;
	// Make sure only supers can reset with a None old_pass.
	if old_pass.is_none() && !user_claims._super {
		return Err(DbError::AuthError);
	}

	let (_, site_id) = db.host_to_site_id(host.hostname());
	let site_id = site_id.ok_or(DbError::NotFound)?;

	db.reset(&user, &pass, old_pass.as_deref(), Some(site_id))
		.map(|_| {
			info!("User password reset '{user}'.");
			log_ip_user("auth_reset", ip.0, &user);
			"User pass reset."
		})
		.map_err(|err| {
			error!("Failed password reset for '{user}': {err:?}.");
			log_ip_user("auth_reset_error", ip.0, &user);
			err
		})
}

pub async fn user(
	Extension(_db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
	ip: ClientIp,
) -> impl IntoResponse {
	log_ip_user("auth_get_user", ip.0, &user_claims.user);
	Json(user_claims)
}

pub async fn user_photo(
	TypedHeader(host): TypedHeader<headers::Host>,
	Path(user): Path<String>,
	Extension(user_claims): Extension<UserClaims>,
	Extension(_db): Extension<LockedAtomic<DBAuth>>,
) -> impl IntoResponse {
	let db = _db.read().unwrap();

	let (_, site_id) = db.host_to_site_id(host.hostname());
	// let site_id = site_id;
	if site_id.is_none() && !user_claims.admin {
		return Err(DbError::NotFound);
	}

	db.user_photo(&user, site_id)
}

/// Allows users to modify certain whitelisted claims.
pub async fn user_patch(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Extension(user_claims): Extension<UserClaims>,
	Json(patch): Json<Value>,
) -> Result<impl IntoResponse, DbError> {
	let (_, mut site_id) = db.read().unwrap().host_to_site_id(host.hostname());
	
	if site_id.is_none() && !user_claims.admin {
		return Err(DbError::NotFound);
	}
	if user_claims.admin {
		site_id = None;
	}
	db.write().unwrap().mod_user_self(site_id, &user_claims.user, patch)?;

	Ok(())
}

pub async fn logout(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(user_claims): Extension<UserClaims>,
	ip: ClientIp,
	headers: HeaderMap,
) -> impl IntoResponse {
	// let host_full = host.hostname();
	let host = hostname_normalize(host.hostname());
	log_ip_user("auth_logout", ip.0, &user_claims.user);

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
			(header::LOCATION, referer.to_string()),
		],
	)
}
