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
	/// Reset a user password
	/// Admins should call with no old_pass to skip password check.
	pub fn reset(&mut self, user: &str, pass: &str, old_pass: Option<&str>, site: Option<SiteId>) -> Result<(), DbError> {
		if let Some(site) = site {
			let mut site = self.sites.get(&site).ok_or(DbError::NotFound)?.write().unwrap();
			let user = site.users.get_mut(user).ok_or(DbError::AuthError)?;
			user.reset_pass(old_pass, pass)
		} else {
			let admin = self.admins.get(user).ok_or(DbError::AuthError)?;
			admin.write().unwrap().user.reset_pass(old_pass, pass)
		}
	}
	/// Try finding user photo in users from provided site
	/// if none are found, search admin users instead
	pub fn user_photo(&self, user: &str, site: Option<SiteId>) -> Result<String, DbError> {
		site
			.and_then(|s| self.sites.get(&s))
			.and_then(|s| s.read().unwrap().users.get(user).map(|u| u.claims.clone()))
			.or_else(|| self.admins.get(user).map(|a| a.read().unwrap().user.claims.clone()))
			.and_then(|c| Some(c.get("photo")?.as_str()?.to_string()))
			.ok_or(DbError::NotFound)
	}
}

#[cfg(test)]
mod tests;
