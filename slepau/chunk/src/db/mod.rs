use common::utils::LockedAtomic;
use serde::{Deserialize, Serialize};
use serde_json::{Value};
/** Designing a new Data Structure that would allow for all queries/insertions/serializations to efficiently happen */
use std::{
	collections::{BTreeMap},
};

pub type DBMap<K, V> = BTreeMap<K, V>;

use self::chunk::Chunk;

/// Graphview allows for a tree structure to be represented
/// - If there's a GraphView, there's a value
/// - If children is Some, then children was queried and will be included
/// - If children is None, then children wasn't meant to to be included
///
/// Meant to be declarative about what was queried or wasn't, as to reduce ambiguity.
#[derive(Serialize, Debug, PartialEq)]
pub struct GraphView(
	Value,
	#[serde(skip_serializing_if = "Option::is_none")] Option<Vec<GraphView>>,
);

/**
 * An improved 2.0, reference counted version,
 * Very much an improvement over the
 * last RAM DB representation that used lookups for everything.
 */
#[derive(Default)]
pub struct DB {
	chunks: DBMap<String, LockedAtomic<dbchunk::DBChunk>>,
}
/**
 * DB data that will acutally get stored on disk
 */
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DBData {
	pub chunks: Vec<Chunk>,
}

pub mod chunk;
pub mod dbchunk;
mod def;
pub mod user_access;
pub mod view;

#[cfg(test)]
mod tests;
