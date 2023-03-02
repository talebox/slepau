use std::collections::{HashMap, VecDeque};

use common::{
	proquint::Proquint,
	utils::{LockedAtomic, LockedWeak},
};
use media::MatcherType;
use serde::{Deserialize, Serialize};

pub mod def;

/// MediaId uses u64 for a max of 2^64 combinations for less collisions.
/// As many as the neurons as 200 million humans combined.
pub type MediaId = Proquint<u64>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Media {
	pub id: MediaId,
	pub name: String,
	pub size: usize,
	#[serde(with = "MatcherType", rename = "type")]
	pub _type: infer::MatcherType,
	pub conversion: ConversionInfo,
}
static CONVERSION_VERSION:usize = 1;
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct ConversionInfo {
	/// How long (in ms)
	time: f32,
	/// How much smaller (in percent)
	size_reduction: f32,
	format: String,
	version: usize,
}

#[derive(Default, Debug)]
pub struct DB {
	conversion_queue: VecDeque<MediaId>,
	conversion_current: Vec<MediaId>,

	// ------------------
	/// Media metadata, holds all metadata about all files.
	///
	/// This is always kept in memory, cause it's small.
	media: HashMap<MediaId, LockedAtomic<Media>>,

	// /// Owner is saved in DB separately, in case multiple owners upload the same media. They will both get to see it.
	by_owner: HashMap<String, Vec<LockedWeak<Media>>>,
	// /// Transcode/Conversion map. An optimization.
	// ///
	// /// This holds the incoming hash -> hash it was converted to.
	// /// So we don't have to do the work of converting/transcoding again.
	// t_map: HashMap<MediaId, LockedWeak<Media>>,
}

#[derive(Serialize)]
pub struct DBStats {
	conversion_queue: usize,
	conversion_current: usize,
	media: usize,
}
impl From<&DB> for DBStats {
	fn from(db: &DB) -> Self {
		Self {
			conversion_queue: db.conversion_queue.len(),
			conversion_current: db.conversion_current.len(),
			media: db.media.len(),
		}
	}
}
