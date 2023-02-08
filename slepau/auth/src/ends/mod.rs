use auth::{validate::KP, UserClaims};
use axum::{extract::Extension, headers, http::header, response::IntoResponse, Json, TypedHeader};
use common::utils::{get_secs, DbError, LockedAtomic, SECS_IN_DAY, SECURE, URL};
use hyper::StatusCode;
use log::{error, info};
use pasetors::{claims::Claims, public};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

// mod admin;

use crate::db::DBAuth;

pub async fn login(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(db): Extension<LockedAtomic<DBAuth>>,
	Json((user, pass)): Json<(String, String)>,
) -> Result<impl IntoResponse, DbError> {
	let db = db.write().unwrap();
	
	let host = psl::domain_str(host.hostname()).ok_or(DbError::InvalidHost)?;
	let site = db.host_to_site_id(host)?;
	
	db.login(&user, &pass, Some(site))
		.map(|user_object| {
			// Create token

			let mut claims = Claims::new().unwrap();

			claims.issuer("slepau:auth").unwrap();

			claims.audience(serde_json::to_string(&host).unwrap().as_str()).unwrap();
			user_object.claims.iter().for_each(|(k, v)| {
				claims.add_additional(k, v.clone());
			});
			claims.add_additional("user", user.clone()).unwrap();

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
			let pub_token = public::sign(&KP.secret, &claims, None, None).unwrap();

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
	
	let host = psl::domain_str(host.hostname()).ok_or(DbError::InvalidHost)?;
	let site = db.host_to_site_id(host)?;
	
	db.new_user(&user, &pass, site)
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
	let site = db.host_to_site_id(host.hostname())?;

	db.reset(&user, &pass, &old_pass, Some(site))
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
