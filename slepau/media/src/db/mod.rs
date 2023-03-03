use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt::Display;
use std::hash::Hash;

use common::{
	proquint::Proquint,
	utils::{LockedAtomic, LockedWeak},
};
use media::MatcherType;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod def;

/// MediaId uses u64 for a max of 2^64 combinations for less collisions.
/// As many as the neurons as 200 million humans combined.
pub type MediaId = Proquint<u64>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Media {
	pub id: MediaId,
	pub name: String,
	pub meta: FileMeta,
	pub versions: HashMap<Version, Option<VersionInfo>>,
}
static CONVERSION_VERSION: usize = 1;
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct VersionInfo {
	/// How long (in ms)
	time: f32,
	/// How much smaller (in percent)
	meta: FileMeta,
	count: usize,
	/// To redo the conversion if version changed.
	version: usize,
}
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
struct FileMeta {
	hash: Proquint<u64>,
	size: usize,
	/// Mime type
	_type: String,
}
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
struct Version(BTreeMap<String, Value>);
impl From<&str> for Version {
	fn from(value: &str) -> Self {
		let v = BTreeMap::<_,_>::from_iter(value.split("&").map(|v| {
			let a = v.split("=").collect::<Vec<_>>();
			if a.len()!=2{panic!("Not valid key=value in Version parsing.");}
			(a[0].into(), serde_json::from_str(a[1]).unwrap())
		}));
		
		Self(v)
	}
}
impl Display for Version {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(
			self
				.0
				.iter()
				.map(|(k, v)| format!("{k}={v}"))
				.collect::<Vec<_>>()
				.join("&")
				.as_str(),
		)
	}
}
impl Hash for Version {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.to_string().hash(state);
	}
}
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
struct Task {
	priority: usize,
	id: MediaId,
	version: Version,
}

#[derive(Default, Debug)]
struct DB {
	media: HashMap<MediaId, Media>,
	by_owner: HashMap<String, HashSet<MediaId>>,

	task_queue: VecDeque<Task>,
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
