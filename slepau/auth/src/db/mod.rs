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
	pub fn host_to_site_id(&self, host: &str) -> Result<SiteId, DbError> {
		self
			.hosts
			.get(host)
			.and_then(|s| s.upgrade().and_then(|s| Some(s.read().unwrap().id)))
			.ok_or(DbError::InvalidHost)
	}

	pub fn login(&self, user: &str, pass: &str, site: Option<SiteId>) -> Result<User, DbError> {
		if let Some(site) = site {
			let site = self.sites.get(&site).ok_or(DbError::InvalidSite)?.write().unwrap();
			let user = site.users.get(user).ok_or(DbError::AuthError)?;
			if !user.verify_pass(pass) {
				return Err(DbError::AuthError);
			}
			Ok(user.clone())
		} else {
			let admin = self.admins.get(user).ok_or(DbError::AuthError)?;
			let user = &admin.read().unwrap().user;
			if !user.verify_pass(pass) {
				return Err(DbError::AuthError);
			}
			Ok(user.clone())
		}
	}
	pub fn reset(&mut self, user: &str, pass: &str, old_pass: &str, site: Option<SiteId>) -> Result<(), DbError> {
		if let Some(site) = site {
			let mut site = self.sites.get(&site).ok_or(DbError::InvalidSite)?.write().unwrap();
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
