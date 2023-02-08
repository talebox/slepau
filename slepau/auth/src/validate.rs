use axum::{
	extract::TypedHeader,
	headers::{Cookie, Host},
	response::IntoResponse,
	RequestPartsExt,
};
use common::utils::{DbError, K_PUBLIC, K_SECRET};
use core::convert::TryFrom;
use hyper::{Method, StatusCode};
use lazy_static::lazy_static;
use pasetors::claims::ClaimsValidationRules;
use pasetors::keys::{AsymmetricKeyPair, AsymmetricPublicKey, AsymmetricSecretKey};
use pasetors::token::{TrustedToken, UntrustedToken};
use pasetors::{public, version4::V4, Public};

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
			"Using keys at K_PUBLIC:'{}', and K_SECRET:'{}'.",
			K_PUBLIC.as_str(),
			K_SECRET.as_str()
		)
	} else {
		panic!(
			"\
			Keys not found at K_PUBLIC:'{}', and K_SECRET:'{}'.\n\
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
pub async fn authenticate<B>(req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
	let mut user_claims = UserClaims {
		user: "public".into(),
		..Default::default()
	};

	let (mut parts, body) = req.into_parts();
	let TypedHeader(host): TypedHeader<Host> = parts.extract().await.expect("A Host header");
	let TypedHeader(cookie): TypedHeader<Cookie> = parts.extract().await.expect("Cookies");
	// reconstruct the request
	let mut req = Request::from_parts(parts, body);
	
	let host = psl::domain_str(host.hostname()).ok_or(StatusCode::NOT_ACCEPTABLE)?;
	let auth_cookie = cookie.get("auth").ok_or(StatusCode::NOT_ACCEPTABLE)?;

	let mut validation_rules = ClaimsValidationRules::new();
	validation_rules.validate_issuer_with("slepau:auth");
	// validation_rules.validate_audience_with(host);

	if let Ok(token) = UntrustedToken::<Public, V4>::try_from(auth_cookie) {
		if let Ok(token) = public::verify(&KP.public, &token, &validation_rules, None, None) {
			user_claims = UserClaims::from(&token.payload_claims().unwrap().clone());
			req.extensions_mut().insert(token);
		}
	}

	req.extensions_mut().insert(user_claims);

	Ok(next.run(req).await)
}

fn get_valid_token(token: &str) -> Option<(TrustedToken, UserClaims)> {
	None
}
