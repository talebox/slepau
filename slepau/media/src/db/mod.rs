use common::{
	proquint::Proquint,
	utils::{LockedAtomic, LockedWeak},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
pub mod def;
pub mod task;

/// MediaId uses u64 for a max of 2^64 combinations for less collisions.
/// As many as the neurons as 200 million humans combined.
pub type MediaId = Proquint<u64>;

#[derive(Serialize, Default, Deserialize, Clone, Debug)]
pub struct Media {
	pub id: MediaId,
	pub name: String,
	pub meta: FileMeta,
	pub versions: HashMap<VersionString, Option<VersionInfo>>,
}

static CONVERSION_VERSION: usize = 1;
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct VersionInfo {
	/// How long (in ms)
	time: f32,
	/// How much smaller (in percent)
	meta: FileMeta,
	count: usize,
}
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct FileMeta {
	hash: Proquint<u64>,
	size: usize,
	/// Mime type
	#[serde(rename = "type")]
	_type: String,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Version {
	#[serde(rename = "type", skip_serializing_if = "Option::is_none")]
	_type: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	xm: Option<usize>,
	#[serde(skip_serializing_if = "Option::is_none")]
	ym: Option<usize>,
}
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VersionString(String);

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Task {
	priority: usize,
	id: MediaId,
	version: VersionString,
}

#[derive(Debug)]
pub struct DB {
	/// Key is matcher, gets applied to whoever's mime type begins with this.
	initial_versions: HashMap<String, HashSet<VersionString>>,

	media: HashMap<MediaId, LockedAtomic<Media>>,
	by_owner: HashMap<String, HashSet<MediaId>>,

	task_queue: VecDeque<Task>,
}
impl Default for DB {
	fn default() -> Self {
		Self {
			initial_versions: serde_json::from_value(json!({"image": ["type=image/webp"]})).unwrap(),
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
