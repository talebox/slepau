pub mod ends;

use ends::MatcherType;
use serde::{Deserialize, Serialize};

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
