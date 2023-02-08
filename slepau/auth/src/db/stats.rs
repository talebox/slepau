use serde::Serialize;

use super::DBAuth;

#[derive(Serialize, PartialEq, Eq, Debug)]
pub struct DBAuthStats {
	pub sites: usize,
	pub hosts: usize,
	pub users: usize,
	pub admins: usize,
}

impl From<&DBAuth> for DBAuthStats {
	fn from(value: &DBAuth) -> Self {
		Self {
			sites: value.sites.len(),
			hosts: value.hosts.len(),
			admins: value.admins.len(),
			users: value.sites.values().fold(0, |a, s| a + s.read().unwrap().users.len()),
		}
	}
}
