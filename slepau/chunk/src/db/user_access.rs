use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Hash, Eq, PartialOrd, Ord, PartialEq, Clone, Debug, Default)]
pub enum Access {
	#[default]
	Read,
	Write,
	Admin,
	Owner,
}
#[derive(Serialize, Deserialize, Hash, Eq, PartialOrd, Ord, PartialEq, Clone, Debug, Default)]
pub struct UserAccess {
	pub user: String,
	pub access: Access,
}
impl From<(String, Access)> for UserAccess {
	fn from((user, access): (String, Access)) -> Self {
		Self { user, access }
	}
}
impl From<(&str, Access)> for UserAccess {
	fn from((user, access): (&str, Access)) -> Self {
		Self::from((user.to_string(), access))
	}
}
impl From<&str> for UserAccess {
	fn from(user: &str) -> Self {
		Self::from((user.to_string(), Access::default()))
	}
}
