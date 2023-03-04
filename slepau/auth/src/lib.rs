use std::collections::HashSet;

use pasetors::claims::Claims;
use serde::{Deserialize, Serialize};

// use serde_json::json;

fn is_false(v: &bool) -> bool {
	!(*v)
}

#[derive(Clone, Deserialize, Serialize, Default)]
#[serde(default)]
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

mod user;
pub mod validate;
