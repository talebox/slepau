use axum::{http::header, response::IntoResponse};
use common::utils::{get_secs, DbError, K_PUBLIC, K_SECRET};
use core::convert::TryFrom;
use hyper::{Method, StatusCode};
use lazy_static::lazy_static;
use log::{info, warn};
use pasetors::claims::ClaimsValidationRules;
use pasetors::keys::{AsymmetricKeyPair, AsymmetricPublicKey, AsymmetricSecretKey, Generate};
use pasetors::token::{TrustedToken, UntrustedToken};
use pasetors::{public, version4::V4, Public};
use std::str::FromStr;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub fn public_key() -> AsymmetricKeyPair<V4> {
	let kp;
	if let (Some(public), Some(secret)) = (
		std::fs::read(K_PUBLIC.as_str())
			.ok()
			.and_then(|b| AsymmetricPublicKey::from(b.as_slice()).ok()),
		std::fs::read(K_SECRET.as_str())
			.ok()
			.and_then(|b| AsymmetricSecretKey::from(b.as_slice()).ok()),
	) {
		kp = AsymmetricKeyPair::<V4> { public, secret };
		println!(
			"Using keys at K_PUBLIC:'{}', and K_SECRET:'{}",
			K_PUBLIC.as_str(),
			K_SECRET.as_str()
		)
	} else {
		panic!(
			"\
			Keys not found at K_PUBLIC:'{}', and K_SECRET:'{}.\n\
			Check they exist or generate them with 'gen_key' otherwise.\n\
			",
			K_PUBLIC.as_str(),
			K_SECRET.as_str()
		)
	}
	kp
}

lazy_static! {
	pub static ref KP: AsymmetricKeyPair::<V4> = public_key();
}

use axum::{http::Request, middleware::Next, response::Response};

use crate::UserClaims;

pub async fn auth_required<B>(req: Request<B>, next: Next<B>) -> Result<Response, impl IntoResponse> {
	if req.extensions().get::<TrustedToken>().is_none() {
		Err(DbError::AuthError)
	} else {
		Ok(next.run(req).await)
	}
}

pub async fn public_only_get<B>(req: Request<B>, next: Next<B>) -> Result<Response, impl IntoResponse> {
	if req.extensions().get::<TrustedToken>().is_none() && *req.method() != Method::GET {
		Err(DbError::AuthError)
	} else {
		Ok(next.run(req).await)
	}
}

/// Function used to authenticate.
pub async fn authenticate<B>(mut req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
	let mut user_claims = UserClaims {
		user: "public".into(),
		..Default::default()
	};

	if let Some(auth_header) = req
		.headers()
		.get(header::COOKIE)
		.and_then(|header| {
			// info!("Header tostr {:?}", header.to_str().ok());
			header.to_str().ok()
		})
		.map(|v| {
			v.split(';').fold(vec![], |mut acc, v| {
				let kv = v.split('=').collect::<Vec<_>>();
				if kv.len() == 2 {
					acc.push((kv[0].trim(), kv[1]))
				}
				acc
			})
		}) {
		if let Some(auth_value) = auth_header.iter().find(|(k, _v)| *k == "auth").map(|v| v.1) {
			if let Some((token, _user_claims)) = get_valid_token(auth_value) {
				let claims = token.payload_claims().unwrap();
				let _user_claim = claims.get_claim("user").unwrap().as_str().unwrap();
				let nbf_claim = claims.get_claim("iat").unwrap().as_str().unwrap();
				let nbf_seconds = OffsetDateTime::parse(nbf_claim, &Rfc3339).unwrap().unix_timestamp() as u64;
				let exp_claim = claims.get_claim("exp").unwrap().as_str().unwrap();
				let exp_seconds = OffsetDateTime::parse(exp_claim, &Rfc3339).unwrap().unix_timestamp() as u64;
				let now = get_secs();
				let mut iat_good = false;

				// let db = req.extensions().get::<LockedAtomic<DBAuth>>().unwrap();
				// if let Ok(user) = db.read().unwrap().get_user(user_claim) {
				// 	iat_good = user.verify_not_before(iat_unix);
				// }
				iat_good = nbf_seconds <= now && now <= exp_seconds; // Simple check

				if iat_good {
					req.extensions_mut().insert(token);
					user_claims = _user_claims;
				} else {
					warn!("Token iat/nbf isn't within an acceptable range for user {_user_claim}");
				}
			}
		}
	}

	req.extensions_mut().insert(user_claims);

	Ok(next.run(req).await)
}

fn get_valid_token(token: &str) -> Option<(TrustedToken, UserClaims)> {
	let mut validation_rules = ClaimsValidationRules::new();
	validation_rules.validate_issuer_with("slepau:auth");

	if let Ok(untrusted_token) = UntrustedToken::<Public, V4>::try_from(token) {
		// if cfg!(debug_assertions) {
		// 	println!("{}", String::from_utf8_lossy(untrusted_token.untrusted_message()));
		// }
		if let Ok(trusted_token) = public::verify(&KP.public, &untrusted_token, &validation_rules, None, None) {
			let claims = trusted_token.payload_claims().unwrap().clone();
			return Some((trusted_token, UserClaims::from(&claims)));
		}
	}

	None
}
