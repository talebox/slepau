use std::collections::HashMap;

use common::utils::{DbError, LockedAtomic, LockedWeak};

use crate::user::User;

use self::site::{Admin, Site, SiteId};
pub mod data;
pub mod site;
pub mod stats;

pub mod delete;
pub mod get;
pub mod modify;
pub mod new;

#[derive(Default)]
pub struct DBAuth {
	/// Site Id -> Site
	pub sites: HashMap<SiteId, LockedAtomic<Site>>,
	/// Host -> Site
	pub hosts: HashMap<String, LockedWeak<Site>>,
	/// UserName -> Admin
	pub admins: HashMap<String, LockedAtomic<Admin>>,
}

impl DBAuth {
	pub fn host_to_site_id(&self, host: &str) -> (String, Option<SiteId>) {
		let host = psl::domain_str(host).unwrap_or(host);
		(
			host.into(),
			self
				.hosts
				.get(host)
				.and_then(|s| s.upgrade().map(|s| s.read().unwrap().id)),
		)
	}

	pub fn login(&self, user: &str, pass: &str, site: Option<SiteId>) -> Result<(User, bool, bool), DbError> {
		let site = site.and_then(|site| self.sites.get(&site));
		if let Some(site) = site {
			if let Some(user) = site.read().unwrap().users.get(user) {
				user.verify_pass(pass)?;
				return Ok((user.clone(), false, false));
			}
		}

		let admin = self.admins.get(user).ok_or(DbError::AuthError)?;
		let admin = admin.read().unwrap();
		let user = &admin.user;
		user.verify_pass(pass)?;
		Ok((user.clone(), true, admin._super))
	}
	pub fn reset(&mut self, user: &str, pass: &str, old_pass: &str, site: Option<SiteId>) -> Result<(), DbError> {
		if let Some(site) = site {
			let mut site = self.sites.get(&site).ok_or(DbError::NotFound)?.write().unwrap();
			let user = site.users.get_mut(user).ok_or(DbError::AuthError)?;
			user.reset_pass(old_pass, pass)
		} else {
			let admin = self.admins.get(user).ok_or(DbError::AuthError)?;

			admin.write().unwrap().user.reset_pass(old_pass, pass)
		}
	}
}

#[cfg(test)]
mod tests;
