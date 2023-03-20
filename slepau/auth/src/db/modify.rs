use std::sync::Arc;

use super::{
	site::{AdminSet, SiteId, SiteSet},
	DBAuth,
};
use crate::user::UserSet;
use common::utils::DbError;
use serde_json::Value;

impl DBAuth {
	pub fn mod_site(&mut self, admin: &str, site_id: SiteId, v: SiteSet) -> Result<(), DbError> {
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

		let site = self.sites.get(&site_id).unwrap();
		let site_weak = Arc::downgrade(site);
		// Remove all hosts that point to the site
		self.hosts.retain(|_, s| !s.ptr_eq(&site_weak));
		// Add new hosts
		self
			.hosts
			.extend(v.hosts.into_iter().map(|h| (h, site_weak.to_owned())));
		
		// Parse claims
		let claims = v
			.claims
			.into_iter()
			.map(|(k, v)| {
				(
					k.to_owned(),
					if let Value::String(s) = &v {
						serde_json::from_str(&s).unwrap_or(v)
					} else {
						v
					},
				)
			})
			.collect();
		
		// Modify site
		{
			let mut site = site.write().unwrap();
			site.max_age = v.max_age;
			site.name = v.name;
			site.claims = claims;
		}
		Ok(())
	}
	pub fn mod_admin(&mut self, super_admin: &str, admin: &str, v: AdminSet) -> Result<(), DbError> {
		// Figure out if it's a super admin
		self
			.admins
			.get(super_admin)
			.and_then(|v| if v.read().unwrap()._super { Some(()) } else { None })
			.ok_or(DbError::AuthError)?;

		// Find admin
		let admin = self.admins.get(admin).ok_or(DbError::AuthError)?;

		let sites = v
			.sites
			.into_iter()
			.map(|site_id| self.sites.get(&site_id).map(Arc::downgrade))
			.collect::<Vec<_>>();
		// If you couldn't find a site, return an error
		if sites.iter().any(|v| v.is_none()) {
			return Err(DbError::InvalidSite(
				"Site not found. Make sure the site id's are correct.",
			));
		}

		// Last minute checks
		{
			let admin = admin.read().unwrap();
			let changing_themselves = super_admin == &admin.user.user;
			let turning_off_super = admin._super && !v._super;
			let making_inactive = admin.user.active && !v.active;
			if changing_themselves {
				if turning_off_super {
					return Err("You can't get rid of your powers.".into());
				}
				if making_inactive {
					return Err("Your power is too strong to be disabled.".into());
				}
			}
		}

		// Modify admin
		{
			let mut admin = admin.write().unwrap();

			admin.user.active = v.active;
			admin.user.claims = v.claims.clone();
			admin.sites = sites
				.into_iter()
				.map(|v| v.expect("Site should be good, we checked ^"))
				.collect();
			admin._super = v._super;
		}

		Ok(())
	}
	pub fn mod_user(&mut self, admin: &str, site_id: SiteId, user: &str, v: UserSet) -> Result<(), DbError> {
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

		// Try parsing the claims as strings
		let claims = v
			.claims
			.into_iter()
			.map(|(k, v)| {
				(
					k.to_owned(),
					if let Value::String(s) = &v {
						serde_json::from_str(&s).unwrap_or(v)
					} else {
						v
					},
				)
			})
			.collect();

		// Modify user
		{
			let mut site = site.write().unwrap();
			let mut user = site.users.get_mut(user).ok_or(DbError::NotFound)?;
			user.active = v.active;
			user.claims = claims;
		}

		Ok(())
	}
}
