use common::{
	proquint::Proquint,
	utils::{LockedAtomic, LockedWeak},
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
	collections::{HashMap, VecDeque},
	path::PathBuf,
};

use self::{
	task::{TaskCriteria, TaskQuery},
	version::VersionString,
};

pub mod def;
pub mod meta;
pub mod task;
pub mod version;
pub mod view;

/// MediaId uses u64 for a max of 2^64 combinations for less collisions.
/// As many as the neurons of 200 million humans combined.
pub type MediaId = Proquint<u64>;

#[derive(Serialize, Default, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Media {
	pub id: MediaId,
	pub name: String,
	pub meta: meta::FileMeta,
	pub versions: HashMap<version::VersionString, VersionInfo>,

	/// Media record creation time (in seconds since epoch)
	///
	/// Note: this is not the image's metadata creation time
	pub created: u64,
}
#[derive(Default, Serialize)]
pub struct MediaStats {
	/// The size, in bytes
	pub size: u64,
}
impl std::ops::Add<Self> for MediaStats {
	type Output = Self;
	fn add(self, rhs: Self) -> Self::Output {
		Self {
			size: self.size + rhs.size,
		}
	}
}
impl From<&LockedWeak<Media>> for MediaStats {
	fn from(value: &LockedWeak<Media>) -> Self {
		value
			.upgrade()
			.map(|v| {
				let v = v.read().unwrap();
				Self {
					size: v.meta.size + v.versions.values().fold(0, |a, v| a + v.meta.size),
				}
			})
			.unwrap_or_default()
	}
}
impl<'a> FromIterator<&'a LockedWeak<Media>> for MediaStats {
	fn from_iter<T: IntoIterator<Item = &'a LockedWeak<Media>>>(iter: T) -> Self {
		iter
			.into_iter()
			.map(Self::from)
			.fold(Default::default(), |a, v| a + v)
	}
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct VersionInfo {
	/// How long (in seconds)
	time: f32,
	/// The metadata of output.
	meta: meta::FileMeta,
	/// How many times has this been accessed.
	count: usize,
	/// Did an error occour, and what was it.
	error: Option<String>,
}

impl From<(MediaId, &PathBuf)> for Media {
	fn from((id, path): (MediaId, &PathBuf)) -> Self {
		Self {
			id,
			meta: meta::FileMeta::from_path(path),
			..Default::default()
		}
	}
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
	/// Should we allow new media by `public` user?
	pub allow_public_post: bool,
	/// Key is matcher, gets applied to whoever's mime type begins with this.
	///
	/// Makes sure entry has X version/s. 
	/// Executed when doing a _tick on the media.
	/// tick_all() ticks all media.
	initial_versions: HashMap<TaskCriteria, Vec<TaskQuery>>,
	/// Key is matcher, gets applied to whoever's mime type begins with this.
	///
	/// On empty query, provide X version.
	default_version: HashMap<TaskCriteria, VersionString>,

	media: HashMap<MediaId, LockedAtomic<Media>>,
	by_owner: HashMap<String, Vec<LockedWeak<Media>>>,

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
			// initial_versions: serde_json::from_value(json!({"video": [{"version":"type=video/webm", "replace": false}]})).unwrap(),
			// default_version: Default::default(),
			default_version: serde_json::from_value(json!({
				"video": "c_v=libsvtav1&c_a=mp3",
				"image": "type=image/webp&max=500_2"
			}))
			.unwrap(),
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
