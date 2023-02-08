use common::utils::{gen_proquint32, get_secs};
use serde::{Deserialize, Serialize};

/**
 * The basic building block
 */
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Chunk {
	pub id: String,
	pub value: String,
	pub owner: String,
	pub created: u64,
	pub modified: u64,
}
impl Default for Chunk {
	fn default() -> Self {
		let secs = get_secs();
		Self {
			id: gen_proquint32(),
			value: Default::default(),
			owner: Default::default(),
			created: secs,
			modified: secs,
		}
	}
}
impl PartialEq for Chunk {
	fn eq(&self, other: &Self) -> bool {
		self.id == other.id && self.owner == other.owner && self.created == other.created
	}
}
/**
 * Value
 */
impl From<&str> for Chunk {
	fn from(value: &str) -> Self {
		Self {
			value: value.to_owned(),
			..Default::default()
		}
	}
}
/**
 * (Id, Value)
 */
impl From<(&str, &str)> for Chunk {
	fn from((id, value): (&str, &str)) -> Self {
		Self {
			id: id.to_owned(),
			value: value.to_owned(),
			..Default::default()
		}
	}
}
/**
 * (Id, Value, Owner)
 */
impl From<(&str, &str, &str)> for Chunk {
	fn from((id, value, owner): (&str, &str, &str)) -> Self {
		Self::from((Some(id), value, owner))
	}
}
/**
 * (Id?, Value, Owner)
 */
impl From<(Option<&str>, &str, &str)> for Chunk {
	fn from((id, value, owner): (Option<&str>, &str, &str)) -> Self {
		Self {
			id: id.map(|v| v.into()).unwrap_or_else(gen_proquint32),
			value: value.to_owned(),
			owner: owner.to_owned(),
			..Default::default()
		}
	}
}
