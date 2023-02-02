use std::{
	collections::{BTreeMap},
	sync::{Arc, RwLock},
};

use common::utils::{DbError, LockedAtomic, KEYWORD_BLACKLIST};
use serde::{Deserialize, Serialize};

use crate::user::User;

#[derive(Default)]
pub struct DBAuth {
	pub users: BTreeMap<String, LockedAtomic<User>>,
}
#[derive(Default, Serialize, Deserialize)]
pub struct DBAuthData {
	pub users: Vec<User>,
}
impl From<&DBAuth> for DBAuthData {
	fn from(value: &DBAuth) -> Self {
		Self {
			users: value.users.values().map(|u| u.read().unwrap().to_owned()).collect(),
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
		DBAuthData::deserialize(deserializer).map(Self::from)
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
	// pub fn get_user(&self, user: &str) -> Result<User, DbError> {
	// 	self
	// 		.users
	// 		.get(user)
	// 		.map(|u| u.read().unwrap().to_owned())
	// 		.ok_or(DbError::NotFound)
	// }
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
