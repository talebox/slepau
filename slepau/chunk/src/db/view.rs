use std::sync::RwLockWriteGuard;

use common::utils::LockedAtomic;
use serde::Serialize;
use serde_json::{Map, Value};

use super::{
	chunk,
	dbchunk::DBChunk,
	user_access::{Access, UserAccess},
};

/**
 * ChunkView is meant for specific Chunk Data
 * It turns an Rc<DBChunk> to an a specific View of it.
 * This will be customizable based on what the UI needs.
 */
#[derive(Serialize, Debug, Default)]
pub struct ChunkView {
	pub id: chunk::ChunkId,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub owner: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub value: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub created: Option<u64>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub modified: Option<u64>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub props: Option<Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub props_dynamic: Option<Value>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub parents: Option<usize>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub children: Option<usize>,

	#[serde(skip_serializing_if = "Option::is_none")]
	pub access: Option<Access>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ViewType {
	Edit,
	Notes,
	Well,
	Graph,
}
impl From<(LockedAtomic<DBChunk>, &str, ViewType)> for ChunkView {
	fn from((rc, user, view_type): (LockedAtomic<DBChunk>, &str, ViewType)) -> Self {
		Self::from((&rc, user, view_type))
	}
}
impl From<(&LockedAtomic<DBChunk>, &str, ViewType)> for ChunkView {
	fn from((rc, user, view_type): (&LockedAtomic<DBChunk>, &str, ViewType)) -> Self {
		let mut db_chunk = rc.write().unwrap();
		let value_short = |db_chunk: &RwLockWriteGuard<DBChunk>| {
			let mut v = 0;
			db_chunk
				.chunk()
				.value
				.chars()
				.take_while(|c| {
					if v == 10 {
						return false;
					};
					if *c == '\n' {
						v += 1;
					};
					true
				})
				.collect::<String>()
		};
		if user == "public" {
			Self {
				id: db_chunk.chunk().id.clone(),
				props: Some(Value::Object(Map::from_iter(db_chunk.props()))),
				value: Some(db_chunk.chunk().value.clone()),
				..Default::default()
			}
		} else {
			match view_type {
				ViewType::Well => Self {
					id: db_chunk.chunk().id.clone(),

					props: Some(Value::Object(Map::from_iter(db_chunk.props()))),
					props_dynamic: Some(Value::Object(Map::from_iter(db_chunk.props_dynamic(&user.into())))),

					value: Some(value_short(&db_chunk)),

					owner: Some(db_chunk.chunk().owner.clone()),
					modified: Some(db_chunk.chunk().modified),
					created: Some(db_chunk.chunk().created),

					parents: Some(db_chunk.parents(Some(&user.into())).len()),
					children: Some(db_chunk.children(Some(&user.into())).len()),

					access: db_chunk
						.highest_access(user)
						.and_then(|a| if a == Access::Owner { None } else { Some(a) }),
					// ..Default::default()
				},
				ViewType::Graph => Self {
					id: db_chunk.chunk().id.clone(),
					created: Some(db_chunk.chunk().created),

					props: Some(Value::Object(Map::from_iter(db_chunk.props()))),
					props_dynamic: Some(Value::Object(Map::from_iter(db_chunk.props_dynamic(&user.into())))),

					parents: Some(db_chunk.parents(Some(&user.into())).len()),
					children: Some(db_chunk.children(Some(&user.into())).len()),
					..Default::default()
				},
				ViewType::Notes => Self {
					id: db_chunk.chunk().id.clone(),
					modified: Some(db_chunk.chunk().modified),

					// props: Some(Value::Object(Map::from_iter(db_chunk.props()))),
					// props_dynamic: Some(Value::Object(Map::from_iter(db_chunk.props_dynamic(&user.into())))),
					value: Some(value_short(&db_chunk)),

					// children: db_chunk.children(Some(&user.into())).len(),
					access: db_chunk
						.highest_access(user)
						.and_then(|a| if a == Access::Owner { None } else { Some(a) }),
					..Default::default()
				},
				ViewType::Edit => Self {
					id: db_chunk.chunk().id.clone(),
					props: Some(Value::Object(Map::from_iter(db_chunk.props()))),
					props_dynamic: Some(Value::Object(Map::from_iter(db_chunk.props_dynamic(&user.into())))),
					// value: Some(db_chunk.chunk().value.clone()),
					owner: Some(db_chunk.chunk().owner.clone()),
					parents: Some(db_chunk.parents(Some(&user.into())).len()),
					children: Some(db_chunk.children(Some(&user.into())).len()),
					modified: Some(db_chunk.chunk().modified),
					created: Some(db_chunk.chunk().created),
					// access: db_chunk
					// 	.highest_access(user)
					// 	.and_then(|a| if a == Access::Owner { None } else { Some(a) }),
					..Default::default()
				},
			}
		}
	}
}
impl From<(LockedAtomic<DBChunk>, &str)> for ChunkView {
	fn from((rc, user): (LockedAtomic<DBChunk>, &str)) -> Self {
		Self::from((rc, user, ViewType::Edit))
	}
}

/**
 * ChunkId is meant for Views
 * It turns an Rc<DBChunk> to an Id String
 */
#[derive(Serialize)]
pub struct ChunkId(chunk::ChunkId);
impl From<LockedAtomic<DBChunk>> for ChunkId {
	fn from(rc: LockedAtomic<DBChunk>) -> Self {
		Self::from(&rc)
	}
}
impl From<&LockedAtomic<DBChunk>> for ChunkId {
	fn from(rc: &LockedAtomic<DBChunk>) -> Self {
		Self(rc.read().unwrap().chunk().id.clone())
	}
}
/**
 * ChunkValue
 * It turns an Rc<DBChunk> to a Value String
 */
#[derive(Serialize)]
pub struct ChunkValue(String);
impl From<LockedAtomic<DBChunk>> for ChunkValue {
	fn from(rc: LockedAtomic<DBChunk>) -> Self {
		Self::from(&rc)
	}
}
impl From<&LockedAtomic<DBChunk>> for ChunkValue {
	fn from(rc: &LockedAtomic<DBChunk>) -> Self {
		Self(rc.read().unwrap().chunk().value.clone())
	}
}
pub enum SortType {
	Modified,
	ModifiedDynamic(UserAccess),
	Created,
}
pub struct ChunkVec(pub Vec<LockedAtomic<DBChunk>>);
impl ChunkVec {
	pub fn sort(&mut self, t: SortType) {
		self.0.sort_by_cached_key(|v| {
			-(match &t {
				SortType::Created => v.read().unwrap().chunk().created,
				SortType::Modified => v.read().unwrap().chunk().modified,
				SortType::ModifiedDynamic(ua) => v.write().unwrap().get_prop_dynamic("modified", ua).unwrap_or_default(),
			} as i64)
		})
	}
}
impl From<Vec<LockedAtomic<DBChunk>>> for ChunkVec {
	fn from(v: Vec<LockedAtomic<DBChunk>>) -> Self {
		Self(v)
	}
}
impl<T: From<LockedAtomic<DBChunk>>> From<ChunkVec> for Vec<T> {
	fn from(val: ChunkVec) -> Self {
		val.0.into_iter().map(|v| v.into()).collect()
	}
}
