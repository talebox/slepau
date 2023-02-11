use common::{proquint::Proquint, utils::LockedWeak};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

use crate::user::User;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct Admin {
	pub user: User,
	pub sites: Vec<LockedWeak<Site>>,
	pub _super: bool,
}
/// This gets serialized,deserialized to Disk
#[derive(Serialize, Deserialize)]
pub struct AdminData {
	pub user: User,
	pub sites: Vec<SiteId>,
	#[serde(rename = "super")]
	pub _super: bool,
}
/// This we user to modify
#[derive(Deserialize)]
pub struct AdminSet {
	pub active: bool,
	pub claims: BTreeMap<String, Value>,
	pub sites: Vec<SiteId>,
	#[serde(rename = "super")]
	pub _super: bool,
}

/// This we user to show
#[derive(Serialize)]
pub struct AdminView {
	user: String,
	pub active: bool,
	pub claims: BTreeMap<String, Value>,
	sites: Vec<SiteId>,
	#[serde(rename = "super")]
	_super: bool,
}

impl From<&Admin> for AdminView {
	fn from(value: &Admin) -> Self {
		Self {
			user: value.user.user.to_owned(),
			active: value.user.active,
			claims: value.user.claims.to_owned(),
			sites: value
				.sites
				.iter()
				.filter_map(|s| s.upgrade().map(|s| s.read().unwrap().id))
				.collect(),
			_super: value._super,
		}
	}
}

pub type SiteId = Proquint<u32>;
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Site {
	pub id: SiteId,
	pub name: String,
	pub users: BTreeMap<String, User>,
	/// Max age for the token (in secs, default is 1d)
	pub max_age: usize,
	/// Can an admin login from this site?
	///
	/// Only `super` users can change this.
	pub allow_admin: bool,
}
impl Default for Site {
	fn default() -> Self {
		Self {
			id: Default::default(),
			users: Default::default(),
			max_age: 60 * 60 * 24,
			name: Default::default(),
			allow_admin: false,
		}
	}
}
#[derive(Deserialize)]
pub struct SiteSet {
	pub name: String,
	pub hosts: Vec<String>,
	pub max_age: usize,
}
#[derive(Serialize)]
pub struct SiteView {
	pub id: SiteId,
	pub name: String,
	pub hosts: Vec<String>,
	pub users: usize,
	pub max_age: usize,
}

impl From<&Site> for SiteView {
	fn from(value: &Site) -> Self {
		Self {
			id: value.id,
			name: value.name.to_owned(),
			users: value.users.len(),
			max_age: value.max_age,
			hosts: Default::default(),
		}
	}
}
