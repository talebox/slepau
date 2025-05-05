use lazy_static::lazy_static;

lazy_static! {
	pub static ref MEDIA_FOLDER: String = std::env::var("MEDIA_FOLDER").unwrap_or_else(|_| "media_".into());
	pub static ref MEDIA_VIDEO_CONVERSION: bool = std::env::var("MEDIA_VIDEO_CONVERSION").unwrap_or_default().parse::<bool>().unwrap_or(true);
	pub static ref MEDIA_IMAGE_CONVERSION: bool = std::env::var("MEDIA_IMAGE_CONVERSION").unwrap_or_default().parse::<bool>().unwrap_or(true);
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(remote = "infer::MatcherType")]
pub enum MatcherType {
	App,
	Archive,
	Audio,
	Book,
	Doc,
	Font,
	Image,
	Text,
	Video,
	Custom,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MediaEntry {
	Ref(String), // Means entry hash maps to another hash, meaning conversion yielded a different hash
	Entry {
		#[serde(skip_serializing_if = "Option::is_none")]
		user: Option<String>,
		#[serde(with = "MatcherType", rename = "type")]
		_type: infer::MatcherType,
	},
}
