use std::collections::HashSet;

use common::proquint::Proquint;
use pasetors::claims::Claims;
use serde::{Deserialize, Serialize};
use serde_json::{json};
use user::User;
// use serde_json::json;

fn is_false(v: &bool) -> bool {
	*v == false
}

#[derive(Clone, Deserialize, Serialize, Default)]
pub struct UserClaims {
	pub user: String,
	
	#[serde(skip_serializing_if = "is_false")]
	pub admin: bool,
	#[serde(rename = "super", skip_serializing_if = "is_false")]
	pub _super: bool,
	
	
	#[serde(skip_serializing_if = "HashSet::is_empty")]
	pub groups: HashSet<String>,
}
impl From<&Claims> for UserClaims {
	fn from(claims: &Claims) -> Self {
		serde_json::from_str(claims.to_string().unwrap().as_str()).unwrap()
	}
}
impl From<&User> for UserClaims {
	fn from(user: &User) -> Self {
		let mut user_claims:UserClaims = serde_json::from_value(json!(user.claims)).unwrap();
		user_claims.user = user.user.to_owned();
		user_claims
	}
}

mod user;
pub mod validate;
