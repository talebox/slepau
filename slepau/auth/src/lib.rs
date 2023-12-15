use pasetors::claims::Claims;
use serde::{Deserialize, Serialize};

// use serde_json::json;

fn is_false(v: &bool) -> bool {
	!(*v)
}
fn is_zero(v: &u64) -> bool {
	*v == 0
}

#[derive(Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct UserClaims {
	pub user: String,
	
	#[serde(skip_serializing_if = "String::is_empty")]
	pub photo: String,

	#[serde(skip_serializing_if = "is_false")]
	pub admin: bool,
	#[serde(rename = "super", skip_serializing_if = "is_false")]
	pub _super: bool,

	/// Media limit, in bytes
	#[serde(skip_serializing_if = "is_zero")]
	pub media_limit: u64,
	
	pub exp: String,
}
impl From<&Claims> for UserClaims {
	fn from(claims: &Claims) -> Self {
		serde_json::from_str(claims.to_string().unwrap().as_str()).unwrap()
	}
}

mod user;
pub mod validate;
