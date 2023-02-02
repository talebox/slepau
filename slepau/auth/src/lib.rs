use std::collections::HashSet;

use pasetors::claims::Claims;
use serde::{Deserialize, Serialize};
use serde_json::json;
use user::User;
// use serde_json::json;

#[derive(Clone, Deserialize, Serialize, Default)]
pub struct UserClaims {
	pub user: String,
	pub groups: HashSet<String>,
}
impl From<&Claims> for UserClaims {
	fn from(claims: &Claims) -> Self {
		serde_json::from_str(claims.to_string().unwrap().as_str()).unwrap()
	}
}
impl From<&User> for UserClaims {
	fn from(value: &User) -> Self {
		serde_json::from_value(json!(value)).unwrap()
	}
}

mod user;
pub mod validate;
