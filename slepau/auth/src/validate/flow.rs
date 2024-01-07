use axum::{
	extract::Request,
	middleware::Next,
	response::{IntoResponse, Response},
};
use common::utils::DbError;
use hyper::Method;
use pasetors::token::TrustedToken;

use crate::UserClaims;

/// Checks user_claim to see if user is super admin, else throw AuthError
pub async fn only_supers(req: Request, next: Next) -> Result<Response, impl IntoResponse> {
	if !req
		.extensions()
		.get::<UserClaims>()
		.map(|claims| claims._super)
		.unwrap_or(false)
	{
		Err(DbError::AuthError)
	} else {
		Ok(next.run(req).await)
	}
}

/// Checks user_claim to see if user is admin, else throw AuthError
pub async fn only_admins(req: Request, next: Next) -> Result<Response, impl IntoResponse> {
	if !req
		.extensions()
		.get::<UserClaims>()
		.map(|claims| claims.admin)
		.unwrap_or(false)
	{
		Err(DbError::AuthError)
	} else {
		Ok(next.run(req).await)
	}
}

/// Checks if a user is logged in, else throw AuthError
pub async fn auth_required(req: Request, next: Next) -> Result<Response, impl IntoResponse> {
	if req.extensions().get::<TrustedToken>().is_none() {
		Err(DbError::AuthError)
	} else {
		Ok(next.run(req).await)
	}
}

/// Only allow public (non-logged) users to use GET requests, else throw AuthError
pub async fn public_only_get(req: Request, next: Next) -> Result<Response, impl IntoResponse> {
	if req.extensions().get::<TrustedToken>().is_none() && *req.method() != Method::GET {
		Err(DbError::AuthError)
	} else {
		Ok(next.run(req).await)
	}
}
