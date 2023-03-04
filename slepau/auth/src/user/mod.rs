use std::collections::BTreeMap;

// use common::utils::get_secs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod blacklist;
mod src;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
	pub user: String,
	pass: String,
	pub active: bool,
	pub claims: BTreeMap<String, Value>,
}
#[derive(Serialize, Clone, Debug)]
pub struct UserView {
	user: String,
	active: bool,
	claims: BTreeMap<String, String>,
}
#[derive(Deserialize)]
pub struct UserSet {
	pub active: bool,
	pub claims: BTreeMap<String, Value>,
}

impl From<&User> for UserView {
	fn from(value: &User) -> Self {
		Self {
			user: value.user.to_owned(),
			active: value.active,
			claims: value
				.claims
				.iter()
				.map(|(k, v)| (k.to_owned(), serde_json::to_string(&v).unwrap()))
				.collect(),
		}
	}
}
