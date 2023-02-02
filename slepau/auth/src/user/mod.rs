use std::collections::HashSet;

// use common::utils::get_secs;
use serde::{Deserialize, Serialize};

mod blacklist;
mod src;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
// #[serde(default)]
pub struct User {
	pub user: String,
	pass: String, // PHC String
	pub groups: HashSet<String>,
}
