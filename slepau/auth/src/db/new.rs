use std::sync::{Arc, RwLock};

use common::utils::DbError;

use crate::user::User;

use super::{
	site::{Admin, Site, SiteId},
	DBAuth,
};

impl DBAuth {
	pub fn new_site(&mut self, admin: &str) -> Result<SiteId, DbError> {
		let site = Site::default();
		let id = site.id;
		let site = Arc::new(RwLock::new(site));
		let admin = self.admins.get(admin).ok_or(DbError::NotFound)?;
		let mut admin = admin.write().unwrap();
		// // Remove dangling/this site
		// admin.sites.retain(|v| v.upgrade().and_then(|v| Some(v.read().unwrap().id != id)).unwrap_or(false));
		admin.sites.push(Arc::downgrade(&site));
		self.sites.insert(id, site);
		Ok(id)
	}
	pub fn new_admin(&mut self, user: &str, pass: &str) -> Result<(), DbError> {
		if self.admins.get(user).is_some() {
			return Err(DbError::UserTaken);
		}
		let admin = Admin {
			user: User::new(user, pass)?,
			sites: Default::default(),
			_super: self.admins.is_empty(),
		};
		self.admins.insert(user.into(), Arc::new(RwLock::new(admin)));
		Ok(())
	}
	pub fn new_user(&mut self, user: &str, pass: &str, site: SiteId) -> Result<(), DbError> {
		let site = self.sites.get(&site).ok_or(DbError::InvalidSite)?;
		if site.read().unwrap().users.get(user).is_some() {
			return Err(DbError::UserTaken);
		}
		let user_instance = User::new(user, pass)?.into();
		site.write().unwrap().users.insert(user.into(), user_instance);
		Ok(())
	}
}
