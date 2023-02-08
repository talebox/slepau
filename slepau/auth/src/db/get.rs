use common::utils::{DataSlice, DbError};

use crate::user::UserView;

use super::{
	site::{AdminView, SiteId, SiteView},
	DBAuth,
};

impl DBAuth {
	pub fn get_admins(&self, super_admin: &str, filter: Option<String>) -> Result<DataSlice<AdminView>, DbError> {
		let admins = self.admins.values().filter_map(|v| {
			let v = v.read().unwrap();
			if filter
				.as_ref()
				.and_then(|filter| Some(v.user.user.contains(filter.as_str())))
				.unwrap_or(true)
			{
				Some(AdminView::from(&*v))
			} else {
				None
			}
		});

		Ok(DataSlice {
			items: admins.clone().take(10).collect(),
			total: admins.clone().count(),
		})
	}

	/// We'll only do get_sites, no specific endpoint by id needed
	pub fn get_sites(&self, admin: &str, filter: Option<String>) -> Result<DataSlice<SiteView>, DbError> {
		let admin = self.admins.get(admin).ok_or(DbError::AuthError)?;
		let admin = admin.read().unwrap();
		let sites = admin.sites.iter().filter_map(|s| s.upgrade()).filter_map(|s| {
			let s = s.read().unwrap();
			if filter
				.as_ref()
				.and_then(|filter| Some(s.name.contains(filter.as_str())))
				.unwrap_or(true)
			{
				let mut sv = SiteView::from(&*s);
				sv.hosts = self
					.hosts
					.iter()
					.filter_map(|(h, site)| {
						if site
							.upgrade()
							.and_then(|_s| Some(_s.read().unwrap().id == s.id))
							.unwrap_or(false)
						{
							Some(h.to_owned())
						} else {
							None
						}
					})
					.collect();
				Some(sv)
			} else {
				None
			}
		});

		Ok(DataSlice {
			items: sites.clone().take(10).collect(),
			total: sites.clone().count(),
		})
	}
	/// We'll only do get_sites, no specific endpoint by id needed
	pub fn get_users(
		&self,
		admin: &str,
		site_id: SiteId,
		filter: Option<String>,
	) -> Result<DataSlice<UserView>, DbError> {
		let admin = self.admins.get(admin).ok_or(DbError::AuthError)?;
		let admin = admin.read().unwrap();
		let site = admin
			.sites
			.iter()
			.filter_map(|s| s.upgrade())
			.find(|s| s.read().unwrap().id == site_id)
			.ok_or(DbError::AuthError)?;
		let site = site.read().unwrap();

		let users = site.users.values().filter_map(|v| {
			// let s = s.read().unwrap();
			if filter
				.as_ref()
				.and_then(|filter| Some(v.user.contains(filter.as_str())))
				.unwrap_or(true)
			{
				Some(UserView::from(&*v))
			} else {
				None
			}
		});

		Ok(DataSlice {
			items: users.clone().take(10).collect(),
			total: users.clone().count(),
		})
	}
}
