use std::collections::HashMap;

use common::utils::{hostname_normalize, DbError, LockedAtomic, LockedWeak};

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
		let host = hostname_normalize(host);
		(
			host.into(),
			self
				.hosts
				.get(host)
				.or_else(|| self.hosts.get("any"))
				.and_then(|s| s.upgrade().map(|s| s.read().unwrap().id)),
		)
	}

	pub fn login(
		&self,
		user: &str,
		pass: &str,
		site: Option<SiteId>,
	) -> Result<(User, Option<Site>, bool, bool, usize), DbError> {
		if let Some(site) = site {
			let site = self.sites.get(&site).ok_or(DbError::InvalidSite("No site found."))?;
			let site = site.read().unwrap();
			let user = site.users.get(user).ok_or(DbError::AuthError)?;
			user.verify_login(pass)?;
			Ok((user.clone(), Some(site.clone()), false, false, site.max_age))
		} else {
			let admin = self.admins.get(user).ok_or(DbError::AuthError)?;
			let admin = admin.read().unwrap();
			let user = &admin.user;
			user.verify_login(pass)?;
			Ok((user.clone(), None, true, admin._super, 60 * 60 * 24))
		}
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
