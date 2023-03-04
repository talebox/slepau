use common::{proquint::Proquint, utils::get_secs};
use serde::{Deserialize, Serialize};

pub type ChunkId = Proquint<u32>;

/**
 * The basic building block
 */
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Chunk {
	pub id: ChunkId,
	pub value: String,
	pub owner: String,
	pub created: u64,
	pub modified: u64,
}
impl Default for Chunk {
	fn default() -> Self {
		let secs = get_secs();
		Self {
			id: Default::default(),
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
impl From<(ChunkId, &str)> for Chunk {
	fn from((id, value): (ChunkId, &str)) -> Self {
		Self {
			id: id,
			value: value.to_owned(),
			..Default::default()
		}
	}
}
/**
 * (Id, Value, Owner)
 */
impl From<(ChunkId, &str, &str)> for Chunk {
	fn from((id, value, owner): (ChunkId, &str, &str)) -> Self {
		Self::from((Some(id), value, owner))
	}
}
/**
 * (Id?, Value, Owner)
 */
impl From<(Option<ChunkId>, &str, &str)> for Chunk {
	fn from((id, value, owner): (Option<ChunkId>, &str, &str)) -> Self {
		Self {
			id: id.unwrap_or_default(),
			value: value.to_owned(),
			owner: owner.to_owned(),
			..Default::default()
		}
	}
}
