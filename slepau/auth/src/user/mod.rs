use std::{
	collections::{HashMap, HashSet},
	sync::Weak,
};

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
	pub claims: HashMap<String, Value>,
}
#[derive(Serialize, Clone, Debug)]
pub struct UserView {
	user: String,
	active: bool,
	claims: HashMap<String, Value>,
}
#[derive(Deserialize)]
pub struct UserSet {
	pub active: bool,
	pub claims: HashMap<String, Value>,
}

impl From<&User> for UserView {
	fn from(value: &User) -> Self {
		Self {
			user: value.user.to_owned(),
			active: value.active,
			claims: value.claims.to_owned(),
		}
	}
}
