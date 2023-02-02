use std::{
	collections::{BTreeMap, HashMap},
	sync::{Arc, RwLock, Weak},
};

use common::utils::{DbError, LockedAtomic, KEYWORD_BLACKLIST};
use serde::{Deserialize, Serialize};

use crate::user::User;

#[derive(Default)]
pub struct DBAuth {
	pub users: BTreeMap<String, LockedAtomic<User>>,

	// /// Groups/roles
	// pub groups: BTreeMap<String, Vec<Weak<RwLock<User>>>>,
}
#[derive(Default, Serialize, Deserialize)]
pub struct DBAuthData {
	/// [User, User, ...]
	pub users: Vec<User>,
	// /// group_name -> [user_id, user_id]
	// pub groups: BTreeMap<String, Vec<String>>,
}
impl From<&DBAuth> for DBAuthData {
	fn from(value: &DBAuth) -> Self {
		Self {
			users: value.users.iter().map(|(id, u)| u.read().unwrap().to_owned()).collect(),
			// groups: value
			// 	.groups
			// 	.iter()
			// 	.map(|v| {
			// 		(
			// 			v.0.to_owned(),
			// 			v.1
			// 				.iter()
			// 				.filter_map(|u| {
			// 					u.upgrade()
			// 						.and_then(|u| u.read().ok().and_then(|u| Some(u.user.to_owned())))
			// 				})
			// 				.collect(),
			// 		)
			// 	})
			// 	.collect(),
		}
	}
}
impl From<DBAuthData> for DBAuth {
	fn from(value: DBAuthData) -> Self {
		let users: BTreeMap<String, LockedAtomic<User>> = value
			.users
			.into_iter()
			.map(|u| (u.user.to_owned(), Arc::new(RwLock::new(u))))
			.collect();
		// let groups = value
		// 	.groups
		// 	.into_iter()
		// 	.map(|(group, user_ids)| {
		// 		(
		// 			group,
		// 			user_ids
		// 				.into_iter()
		// 				.filter_map(|id| users.get(&id).and_then(|u| Some(Arc::downgrade(u))))
		// 				.collect(),
		// 		)
		// 	})
		// 	.collect();

		Self { users }
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
		DBAuthData::deserialize(deserializer).and_then(|v| Ok(Self::from(v)))
	}
}

impl DBAuth {
	pub fn new_user(&mut self, user: &str, pass: &str) -> Result<(), DbError> {
		if self.users.get(user).is_some() {
			return Err(DbError::UserTaken);
		}
		if KEYWORD_BLACKLIST.iter().any(|ub| user.contains(ub)) {
			return Err(DbError::InvalidUsername);
		}

		let user_instance = User::new(user, pass)?;

		self.users.insert(user.into(), Arc::new(RwLock::new(user_instance)));

		Ok(())
	}
	pub fn get_user(&self, user: &str) -> Result<User, DbError> {
		self
			.users
			.get(user)
			.map(|u| u.read().unwrap().to_owned())
			.ok_or(DbError::NotFound)
	}
	pub fn login(&self, user: &str, pass: &str) -> Result<User, DbError> {
		let user = self.users.get(user).ok_or(DbError::AuthError)?.read().unwrap();
		if !user.verify_pass(pass) {
			return Err(DbError::AuthError);
		}
		Ok(user.clone())
	}
	pub fn reset(&mut self, user: &str, pass: &str, old_pass: &str) -> Result<(), DbError> {
		let mut user = self.users.get(user).ok_or(DbError::AuthError)?.write().unwrap();

		user.reset_pass(old_pass, pass)
	}
}

#[cfg(test)]
mod tests;
