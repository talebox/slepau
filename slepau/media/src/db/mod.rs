use common::{
	proquint::Proquint,
	utils::{get_hash, LockedAtomic, LockedWeak},
};
use proquint::Quintable;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
pub mod def;
pub mod task;
pub mod view;

/// MediaId uses u64 for a max of 2^64 combinations for less collisions.
/// As many as the neurons as 200 million humans combined.
pub type MediaId = Proquint<u64>;

#[derive(Serialize, Default, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Media {
	pub id: MediaId,
	pub name: String,
	pub meta: FileMeta,
	pub versions: HashMap<VersionString, VersionInfo>,
	/// Media record creation time
	///
	/// Note: this is not the image's metadata creation time
	pub created: u64,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct VersionInfo {
	/// How long (in seconds)
	time: f32,
	meta: FileMeta,
	count: usize,
}
use base64::Engine as _;
// type my_engine = base64::engine::general_purpose;
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Exif(String);
impl Exif {
	pub fn to_exif(&self) -> exif::Exif {
		let r = exif::Reader::new();
		r.read_raw(
			base64::engine::general_purpose::STANDARD_NO_PAD
				.decode(self.0.clone())
				.unwrap(),
		)
		.unwrap()
	}
	pub fn from_img(value: &Vec<u8>) -> Option<Self> {
		let reader = exif::Reader::new();
		let mut b = std::io::BufReader::new(std::io::Cursor::new(value));
		reader
			.read_from_container(&mut b)
			.map(|v| Self(base64::engine::general_purpose::STANDARD_NO_PAD.encode(v.buf().to_owned())))
			.ok()
	}
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct FileMeta {
	hash: Proquint<u64>,
	size: usize,
	/// Mime type
	#[serde(rename = "type")]
	_type: String,
	exif: Option<Exif>,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskCriteria(String);
impl TaskCriteria {
	fn matches(&self, media: &Media) -> bool {
		media.meta._type.starts_with(&self.0)
	}
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Version {
	#[serde(rename = "type", skip_serializing_if = "Option::is_none")]
	_type: Option<String>,
	// #[serde(skip_serializing_if = "Option::is_none")]
	// xm: Option<usize>,
	// #[serde(skip_serializing_if = "Option::is_none")]
	// ym: Option<usize>,
	/// Defines a max ^2 squared size for the image.
	///
	/// Such a way that if max = 100, that means image will be capped at 100*100 px.
	/// That means image can be 10 * 1000, or 1 * 10000, this cap is only pixel-wize.
	#[serde(skip_serializing_if = "Option::is_none")]
	max: Option<usize>,
}
/// Encodes a version as a string.
///
/// The encoding should be normalized so if two versions have the same data, they are the same.
#[derive(Default, Serialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VersionString(String);
// pub type VersionString = String;

/// A version reference has everything needed to figure out a path to the data.
///
/// It's the combination of Media + Version
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VersionReference {
	id: MediaId,
	version: VersionString,
}
impl From<(MediaId, VersionString)> for VersionReference {
	fn from((id, version): (MediaId, VersionString)) -> Self {
		Self { id, version }
	}
}
impl VersionReference {
	pub fn to_filename(&self) -> String {
		get_hash(self).to_quint()
	}
}

#[derive(Default, Clone, Debug)]
pub struct Task {
	pub priority: usize,
	pub _ref: VersionReference,
}

#[derive(Debug)]
pub struct DB {
	/// Allows new media by public user
	allow_public_post: bool,
	/// Key is matcher, gets applied to whoever's mime type begins with this.
	initial_versions: HashMap<TaskCriteria, HashSet<VersionString>>,

	media: HashMap<MediaId, LockedAtomic<Media>>,
	by_owner: HashMap<String, HashSet<MediaId>>,

	task_queue: VecDeque<Task>,
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
