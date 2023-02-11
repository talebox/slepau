use common::utils::DbError;

use super::{site::SiteId, DBAuth};

impl DBAuth {
	pub fn del_admin(&mut self, super_admin: &str, admin: &str) -> Result<(), DbError> {
		// Make sure it's a super admin
		self
			.admins
			.get(super_admin)
			.and_then(|v| if v.read().unwrap()._super { Some(()) } else { None })
			.ok_or(DbError::AuthError)?;

		// Figure out if it's an admin
		self.admins.remove(admin).map(|_| ()).ok_or(DbError::NotFound)
	}
	
	pub fn del_site(&mut self, admin: &str, site_id: SiteId) -> Result<(), DbError> {
		// Figure out if site belongs to user in question, or we're super admins
		let admin = self.admins.get(admin).ok_or(DbError::AuthError)?;
		{
			let admin = admin.read().unwrap();
			if !admin._super
				&& !admin
					.sites
					.iter()
					.filter_map(|v| v.upgrade())
					.any(|v| v.read().unwrap().id == site_id)
			{
				return Err(DbError::AuthError);
			}
		}

		// Remove said site
		self.sites.remove(&site_id);

		Ok(())
	}

	pub fn del_user(&mut self, admin: &str, site_id: SiteId, user: &str) -> Result<(), DbError> {
		// Find admin
		let admin = self.admins.get(admin).ok_or(DbError::AuthError)?;
		let admin = admin.read().unwrap();
		// Find site
		let site = admin
			.sites
			.iter()
			.filter_map(|v| v.upgrade())
			.find(|v| v.read().unwrap().id == site_id)
			.ok_or(DbError::NotFound)?;

		// Remove user
		site.write().unwrap().users.remove(user).ok_or(DbError::NotFound)?;

		Ok(())
	}
}
