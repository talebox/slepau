use axum::{
	extract::TypedHeader,
	headers::{self, Cookie, Host},
	http::Request,
	middleware::Next,
	response::{IntoResponse, Response},
	Extension, RequestPartsExt,
};
use common::utils::{hostname_normalize, K_PRIVATE, K_PUBLIC, K_SECRET, WEB_DIST};
use core::convert::TryFrom;
use hyper::{header, StatusCode};
use lazy_static::lazy_static;
use pasetors::{
	claims::ClaimsValidationRules,
	keys::{AsymmetricKeyPair, AsymmetricPublicKey, AsymmetricSecretKey, SymmetricKey},
	local,
	token::UntrustedToken,
	version4::V4,
	Local,
};
use std::path::PathBuf;

lazy_static! {
	pub static ref KPR: SymmetricKey::<V4> = private_key();
	pub static ref KP: AsymmetricKeyPair::<V4> = public_key();
}

use crate::UserClaims;

pub mod flow;

fn private_key() -> SymmetricKey<V4> {
	let kp;
	if let Some(private) = std::fs::read(K_PRIVATE.as_str())
		.ok()
		.and_then(|b| SymmetricKey::<V4>::from(b.as_slice()).ok())
	{
		kp = private;
		println!("Using key at K_PRIVATE:'{}'.", K_PRIVATE.as_str(),)
	} else {
		panic!(
			"\
			Keys not found at K_PRIVATE:'{}'.\n\
			Check it exists or generate it with 'gen_key' otherwise.\n\
			",
			K_PRIVATE.as_str(),
		)
	}
	kp
}

fn public_key() -> AsymmetricKeyPair<V4> {
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

/// Function used to authenticate.
pub async fn authenticate<B>(req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
	let mut user_claims = UserClaims {
		user: "public".into(),
		..Default::default()
	};

	let (mut parts, body) = req.into_parts();
	let TypedHeader(host): TypedHeader<Host> = parts.extract().await.expect("A Host header");
	let cookies = parts.extract::<TypedHeader<Cookie>>().await;
	let mut req = Request::from_parts(parts, body);

	let host = host.hostname();
	let host = hostname_normalize(host);

	if let Some(auth_cookie) = cookies
		.ok()
		.and_then(|cookies| cookies.get("auth").map(|v| v.to_owned()))
	{
		let mut validation_rules = ClaimsValidationRules::new();
		validation_rules.validate_issuer_with("slepau:auth");
		validation_rules.validate_audience_with(host);

		if let Ok(token) = UntrustedToken::<
			// Public
			Local,
			V4,
		>::try_from(&auth_cookie)
		{
			if let Ok(token) =
				// public::verify(&KP.public, &token, &validation_rules, None, None)
				local::decrypt(&KPR, &token, &validation_rules, None, None)
			{
				user_claims = UserClaims::from(&token.payload_claims().unwrap().clone());
				req.extensions_mut().insert(token);
			}
		}
	}

	req.extensions_mut().insert(user_claims);

	Ok(next.run(req).await)
}

pub async fn index_service_user(
	TypedHeader(host): TypedHeader<headers::Host>,
	Extension(claims): Extension<UserClaims>,
) -> Result<impl IntoResponse, impl IntoResponse> {
	let host = hostname_normalize(host.hostname());

	fn get_src() -> Option<String> {
		std::fs::read_to_string(PathBuf::from(WEB_DIST.as_str()).join("index.html")).ok()
	}

	let src;
	// If we're debugging, get home every time
	if cfg!(debug_assertions) {
		src = get_src();
	} else {
		lazy_static! {
			static ref HOME: Option<String> = get_src();
		}
		src = HOME.to_owned();
	}

	src
		.as_ref()
		.map(|home| {
			(
				[(header::CONTENT_TYPE, "text/html")],
				home
					.replace("_HOST_", serde_json::to_string(host).unwrap().as_str())
					.replace("_USER_", serde_json::to_string(&claims).unwrap().as_str()),
			)
		})
		.ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}
