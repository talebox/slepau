use common::{
	proquint::Proquint,
	utils::{DataSlice, DbError},
};
use serde::Deserialize;

use crate::user::UserView;

use super::{
	site::{AdminView, SiteId, SiteView},
	DBAuth,
};

#[derive(Deserialize)]
#[serde(untagged)]
enum AnyFilter {
	Id(Proquint<u32>),
	String(String),
}
#[derive(Deserialize, Default)]
pub struct Filter {
	any: Option<AnyFilter>,
}

impl DBAuth {
	pub fn get_admins(&self, super_admin: &str, filter: Filter) -> Result<DataSlice<AdminView>, DbError> {
		// Make sure it's a super admin
		self
			.admins
			.get(super_admin)
			.and_then(|v| if v.read().unwrap()._super { Some(()) } else { None })
			.ok_or(DbError::AuthError)?;

		// Make a view of admins
		let admins = self.admins.values().filter_map(|v| {
			let v = v.read().unwrap();
			if filter
				.any
				.as_ref()
				.map(|filter| match filter {
					AnyFilter::Id(_id) => true,
					AnyFilter::String(name) => v.user.user.contains(name.as_str()),
				})
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
	pub fn get_sites(&self, admin: &str, filter: Filter) -> Result<DataSlice<SiteView>, DbError> {
		let admin = self.admins.get(admin).ok_or(DbError::AuthError)?;
		let admin = admin.read().unwrap();
		let sites = admin.sites.iter().filter_map(|s| s.upgrade()).filter_map(|s| {
			let s = s.read().unwrap();

			if filter
				.any
				.as_ref()
				.map(|filter| match filter {
					AnyFilter::Id(id) => s.id == *id,
					AnyFilter::String(name) => s.name.contains(name.as_str()),
				})
				.unwrap_or(true)
			{
				let mut sv = SiteView::from(&*s);
				sv.hosts = self
					.hosts
					.iter()
					.filter_map(|(h, site)| {
						if site.upgrade().map(|_s| _s.read().unwrap().id == s.id).unwrap_or(false) {
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
	pub fn get_users(&self, admin: &str, site_id: SiteId, filter: Filter) -> Result<DataSlice<UserView>, DbError> {
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
				.any
				.as_ref()
				.map(|filter| match filter {
					AnyFilter::Id(_id) => true,
					AnyFilter::String(name) => v.user.contains(name.as_str()),
				})
				.unwrap_or(true)
			{
				Some(UserView::from(v))
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
