use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	sync::{Arc, RwLock},
};

use super::{
	site::{Admin, AdminData, Site, SiteId},
	DBAuth,
};

#[derive(Default, Serialize, Deserialize)]
pub struct DBAuthData {
	pub sites: Vec<Site>,
	pub hosts: Vec<(String, SiteId)>,
	pub admins: Vec<AdminData>,
}

impl From<&DBAuth> for DBAuthData {
	fn from(value: &DBAuth) -> Self {
		Self {
			sites: value.sites.values().map(|u| u.read().unwrap().to_owned()).collect(),
			admins: value
				.admins
				.values()
				.map(|a| {
					let r = a.read().unwrap().to_owned();
					AdminData {
						user: r.user,
						sites: r
							.sites
							.into_iter()
							.filter_map(|s| s.upgrade().and_then(|s| Some(s.read().unwrap().id)))
							.collect(),
						_super: r._super,
					}
				})
				.collect(),
			hosts: value
				.hosts
				.iter()
				.filter_map(|(h, s)| s.upgrade().and_then(|s| Some((h.to_owned(), s.read().unwrap().id))))
				.collect(),
		}
	}
}
impl From<DBAuthData> for DBAuth {
	fn from(value: DBAuthData) -> Self {
		let sites: HashMap<SiteId, Arc<RwLock<Site>>> = value
			.sites
			.into_iter()
			.map(|u| (u.id.to_owned(), Arc::new(RwLock::new(u))))
			.collect();
		let admins = value
			.admins
			.into_iter()
			.map(|u| {
				(
					u.user.user.to_owned(),
					Arc::new(RwLock::new(Admin {
						user: u.user,
						sites: u
							.sites
							.into_iter()
							.map(|id| Arc::downgrade(sites.get(&id).unwrap()))
							.collect(),
						_super: u._super,
					})),
				)
			})
			.collect();

		let hosts = value
			.hosts
			.into_iter()
			.map(|(h, id)| (h, Arc::downgrade(sites.get(&id).unwrap())))
			.collect();

		Self { sites, admins, hosts }
	}
}
impl Serialize for DBAuth {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		DBAuthData::from(self).serialize(serializer)
	}
}
impl<'de> Deserialize<'de> for DBAuth {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		DBAuthData::deserialize(deserializer).map(Self::from)
	}
}
