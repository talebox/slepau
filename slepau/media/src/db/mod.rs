use common::{
	proquint::Proquint,
	utils::{get_hash, LockedAtomic, LockedWeak},
};
use proquint::Quintable;
use serde::{Deserialize, Serialize};
use std::collections::{ HashMap, HashSet, VecDeque};

pub mod def;
pub mod meta;
pub mod task;
pub mod version;
pub mod view;

/// MediaId uses u64 for a max of 2^64 combinations for less collisions.
/// As many as the neurons as 200 million humans combined.
pub type MediaId = Proquint<u64>;

#[derive(Serialize, Default, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Media {
	pub id: MediaId,
	pub name: String,
	pub meta: meta::FileMeta,
	pub versions: HashMap<version::VersionString, VersionInfo>,
	/// Media record creation time
	///
	/// Note: this is not the image's metadata creation time
	pub created: u64,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct VersionInfo {
	/// How long (in seconds)
	time: f32,
	meta: meta::FileMeta,
	count: usize,
}
impl From<&Vec<u8>> for Media {
	fn from(value: &Vec<u8>) -> Self {
		Self {
			meta: value.into(),
			..Default::default()
		}
	}
}
impl From<(MediaId, &Vec<u8>)> for Media {
	fn from((id, value): (MediaId, &Vec<u8>)) -> Self {
		Self {
			id,
			meta: value.into(),
			..Default::default()
		}
	}
}

#[derive(Debug)]
pub struct DB {
	/// Allows new media by public user
	allow_public_post: bool,
	/// Key is matcher, gets applied to whoever's mime type begins with this.
	initial_versions: HashMap<task::TaskCriteria, HashSet<version::VersionString>>,

	media: HashMap<MediaId, LockedAtomic<Media>>,
	by_owner: HashMap<String, HashSet<MediaId>>,

	task_queue: VecDeque<task::Task>,
}
impl DB {
	pub fn tasks_len(&self) -> usize {
		self.task_queue.len()
	}
}
impl Default for DB {
	fn default() -> Self {
		Self {
			allow_public_post: false,
			initial_versions: Default::default(),
			// initial_versions: serde_json::from_value(json!({"image": ["type=image/webp"]})).unwrap(),
			media: Default::default(),
			by_owner: Default::default(),
			task_queue: Default::default(),
		}
	}
}

#[derive(Serialize)]
pub struct DBStats {
	task_queue: usize,
	// conversion_current: usize,
	media: usize,
}
impl From<&DB> for DBStats {
	fn from(db: &DB) -> Self {
		Self {
			task_queue: db.task_queue.len(),
			media: db.media.len(),
		}
	}
}
