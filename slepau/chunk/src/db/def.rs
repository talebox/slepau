use common::utils::{diff_calc, DbError, LockedAtomic, LockedWeak};
/**
 * A DB without a reference (normalized title) implementation and actual dynamic memory pointers instead of repetitive lookups.
 * Should be orders of magnitud simpler and faster.
 */
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
	collections::HashSet,
	sync::{Arc, RwLock},
};

use super::{
	chunk::{Chunk, ChunkId},
	dbchunk::DBChunk,
	user_access::{Access, UserAccess},
	DBMap, GraphView, DB,
};

/**
 * DB data that will acutally get stored on disk
 */
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DBData {
	pub chunks: Vec<Chunk>,
}

// impl From<DBData> for DB {
// 	fn from(value: DBData) -> Self {
// 		let chunks = value
// 			.chunks
// 			.into_iter()
// 			.map(|chunk| (chunk.id.clone(), (chunk.clone(), ChunkMeta::from(&chunk.value))))
// 			.collect::<HashMap<String, ChunkAndMeta>>();
// 		// Ref->id on conversion
// 		let mut ref_id = HashMap::<String, Vec<String>>::default();
// 		chunks.iter().for_each(|(id, (_, meta))| {
// 			ref_id
// 				.entry(meta._ref.to_owned())
// 				.and_modify(|v| v.push(id.to_owned()))
// 				.or_insert(vec![id.to_owned()]);
// 		});

// 		DB {
// 			chunks,
// 			users: value.users.into_iter().map(|user| (user.user.clone(), user)).collect(),
// 			ref_id,
// 		}
// 	}
// }

impl DB {
	/// Goes through tree and creates a GraphView
	///
	/// - If root=None, iter=0 --> [Value::Null ] // We pull nothing
	/// - If root=None, iter=1 --> [Value::Null, [ ["basab"] ] ] // We pull 1rst level
	/// - If root=None, iter=2 --> [Value::Null, [ ["basab", [["basorb"]] ] ] ] // We pull 2nd level
	pub fn subtree<CF, VF>(
		&self,
		root: Option<&LockedAtomic<DBChunk>>,
		ua: &UserAccess,
		// Function that modifies children, perhaps to sort them
		children_fn: &CF,
		// Function that turns Node -> View
		view_fn: &VF,
		iter: i32,
	) -> GraphView
	where
		CF: Fn(Vec<LockedAtomic<DBChunk>>) -> Vec<LockedAtomic<DBChunk>>,
		VF: Fn(&LockedAtomic<DBChunk>) -> Value,
	{
		// public assertion
		if ua.user == "public" {
			return GraphView(Value::Null, None);
		}

		if let Some(chunk) = root {
			GraphView(
				view_fn(chunk),
				if iter > 0 {
					let c = chunk.read().unwrap().children(Some(ua));
					Some(
						children_fn(c)
							.into_iter()
							.map(|root| self.subtree(Some(&root), ua, children_fn, view_fn, iter - 1))
							.collect(),
					)
				} else {
					None
				},
			)
		} else {
			GraphView(
				Value::Null,
				if iter > 0 {
					Some(
						children_fn(
							self
								.chunks
								.values()
								.filter_map(|v| {
									let mut g = false;
									{
										if let Ok(chunk) = v.read() {
											if chunk.has_access(ua) && chunk.parents(Some(ua)).is_empty() {
												g = true;
											}
										}
									}
									if g {
										Some(v.to_owned())
									} else {
										None
									}
								})
								.collect(),
						)
						.into_iter()
						.map(|root| self.subtree(Some(&root), ua, children_fn, view_fn, iter - 1))
						.collect(),
					)
				} else {
					None
				},
			)
		}
	}

	pub fn get_chunks(&mut self, user: &str) -> Vec<LockedAtomic<DBChunk>> {
		// public assertion
		if user == "public" {
			return vec![];
		}

		self
			.chunks
			.values()
			.filter_map(|v| {
				if let Ok(chunk) = v.write() {
					if chunk.has_access(&user.into()) {
						return Some(v.clone());
					}
				}
				None
			})
			.collect()
	}

	///  Gets a chunk by id
	pub fn get_chunk(&self, id: ChunkId, user: &str) -> Option<LockedAtomic<DBChunk>> {
		self.chunks.get(&id).and_then(|chunk_ref| {
			let chunk = chunk_ref.write().unwrap();
			if chunk.has_access(&user.into()) || chunk.is_public() {
				Some(chunk_ref.clone())
			} else {
				None
			}
		})
	}

	/// Deletes a chunk by id, returns list of users for which access changed

	pub fn del_chunk(&mut self, ids: HashSet<ChunkId>, user: &str) -> Result<HashSet<String>, DbError> {
		// public assertion
		if user == "public" {
			error!("Public tried to delete {:?}", &ids);
			return Err(DbError::AuthError);
		}

		let mut changed = HashSet::<String>::default();
		let mut to_remove = HashSet::<ChunkId>::default();

		for id in ids {
			// Temporary variables for update
			let mut chunk_to_replace = None;
			if let Some(chunk_ref) = self.chunks.get(&id) {
				let chunk = chunk_ref.write().unwrap();
				if chunk.has_access(&(user.to_owned(), Access::Admin).into()) {
					to_remove.insert(chunk.chunk().id.to_owned());
					changed.extend(chunk.access_diff(None));
				} else if chunk.has_access(&user.into()) {
					// Have to think about this a bit more, specially when concerning groups
					// If a user has read access and he/she is part of a group there has to be a way for them to exit out...
					let mut chunk = DBChunk::from((id, chunk.chunk().value.as_str(), chunk.chunk().owner.as_str()));
					let mut access = chunk
						.get_prop::<HashSet<UserAccess>>("access")
						.expect("If user has read access, access has to be valid here");
					access.retain(|ua| ua.user != user); // Remove all of this users's access
					if !chunk.r#override("access", json!(access)) {
						error!("Couldn't do shit here");
						return Err(DbError::AuthError);
					};
					chunk_to_replace = Some(chunk);
				} else {
					return Err(DbError::AuthError);
				}
			} else {
				return Err(DbError::NotFound);
			}
			// Perform the update
			if let Some(chunk_to_replace) = chunk_to_replace {
				let owner = chunk_to_replace.chunk().owner.clone();
				self.set_chunk(chunk_to_replace, owner.as_str()).unwrap();

				changed.insert(user.into());
			}
		}

		// Delete all them chunks which have to be deleted
		to_remove.iter().for_each(|id| {
			{
				// Invalidate all parents
				self.chunks.get(id).unwrap().write().unwrap().invalidate(&vec![], true)
			}
			self.chunks.remove(id);
		});

		Ok(changed)
	}
	/// Receives a Chunk which it validates & links, returns the list of users for which access changed
	///
	pub fn set_chunk(&mut self, mut chunk: DBChunk, user: &str) -> Result<HashSet<String>, DbError> {
		// public assertion
		if user == "public" {
			error!("Public can't set/modify a chunk.");
			return Err(DbError::AuthError);
		}

		let diff_users;
		let diff_props;
		if let Some(chunk_old) = self.chunks.get(&chunk.chunk().id).cloned() {
			// Updating
			let chunk_old = chunk_old.write().unwrap();

			// Perform update check
			if !chunk_old.try_clone_to(&mut chunk, user) {
				return Err(DbError::AuthError);
			}

			// Find diff, link and insert
			diff_users = chunk_old.access_diff(Some(&chunk));
			diff_props = chunk_old.props_diff(Some(&chunk));
		} else {
			// Creating
			// If creating a chunk, user has to be same as Chunk owner
			chunk.set_owner(user.to_owned());

			// Find diff, link and insert
			diff_users = chunk.access_diff(None);
			diff_props = chunk.props_diff(None);
		}

		let id = chunk.chunk().id.clone();
		let chunk = Arc::new(RwLock::new(chunk));
		self.link_chunk(&chunk, None)?;
		{
			let mut chunk = chunk.write().unwrap();
			chunk.invalidate(&vec!["modified"], true);
		}

		self.chunks.insert(id, chunk);

		Ok(diff_users)
	}
	/// Chunk update called by socket, adds `diff` information to returned Result
	pub fn update_chunk(
		&mut self,
		chunk: DBChunk,
		user: &str,
	) -> Result<(HashSet<String>, Vec<String>, LockedAtomic<DBChunk>), DbError> {
		if let Some(last_value) = self
			.get_chunk(chunk.chunk().id, user)
			.map(|v| v.read().unwrap().chunk().value.to_owned())
		{
			let value = chunk.chunk().value.clone();
			let id = chunk.chunk().id.clone();
			let users_to_notify = self.set_chunk(chunk, user)?;
			let diff = diff_calc(&last_value, &value);
			let db_chunk = self.get_chunk(id, user).unwrap();
			return Ok((users_to_notify, diff, db_chunk));
		}
		Err(DbError::NotFound)
	}
	pub fn link_all(&mut self) -> Result<(), DbError> {
		let chunks = self.chunks.values().cloned().collect::<Vec<_>>();
		for chunk in chunks {
			self.link_chunk(&chunk, None)?;
		}
		Ok(())
	}

	/// Processes a chunk within the tree. Making sure there are no circular references.
	/// Recursively calls itself for every parent found
	///
	/// Description.
	///
	/// * `chunk` - The chunk that's currently being linked
	/// * `child` - If None, `chunk` is the original, Some if its a recursive iteration and we're checking for circulars.
	fn link_chunk(
		&mut self,
		chunk: &LockedAtomic<DBChunk>,
		child: Option<&LockedAtomic<DBChunk>>,
	) -> Result<(), DbError> {
		// Detect circular reference
		if let Some(child) = child {
			// If child was Some, means this is a recursive iteration
			if Arc::ptr_eq(chunk, child) {
				// println!("Circular reference detected!");
				return Err(DbError::InvalidChunk("Circular reference not allowed!"));
			}
		}

		// Link parents and tell parents about us if we haven't already
		{
			let mut chunk_lock = chunk.try_write().unwrap();
			if !chunk_lock.linked {
				// Link parents by matching ids to existing chunks
				if let Some(parent_ids) = chunk_lock.get_prop::<Vec<ChunkId>>("parents") {
					if parent_ids.contains(&chunk_lock.chunk().id) {
						// error!("Circular reference detected!; Links to itself");
						return Err(DbError::InvalidChunk("Links to itself not allowed!"));
					}

					let parent_weaks = parent_ids
						.iter()
						.filter_map(|id| self.chunks.get(id).map(Arc::downgrade));

					chunk_lock.parents.extend(parent_weaks);
				}
				// Tell those parents that this is one of their children
				chunk_lock.parents(None).iter().for_each(|v| {
					if let Ok(mut v) = v.write() {
						v.link_child(chunk);
					}
				});
				// Tell those children that this is one of their parents
				chunk_lock.children(None).iter().for_each(|v| {
					if let Ok(mut v) = v.write() {
						v.link_parent(chunk);
					}
				});

				chunk_lock.linked = true;
			}
		}

		// Keep detecting any circular reference, by recursing all parents
		{
			let parents = chunk.read().unwrap().parents(None);
			for parent in parents {
				// Iterate through all parents, linking + checking for circularity
				let child = child.unwrap_or(chunk);
				// println!("Iterate chunk {} child {:?}", parent.read().unwrap().chunk().id, Arc::as_ptr(child));
				self.link_chunk(&parent, Some(child))?;
			}
		}

		Ok(())
	}
}

/**
 * Creates a base implementation of RAM data from what was saved
 */
impl From<DBData> for DB {
	fn from(data: DBData) -> Self {
		let mut by_owner: DBMap<String, Vec<LockedWeak<DBChunk>>> = Default::default();
		let chunks: DBMap<ChunkId, LockedAtomic<DBChunk>> = data
			.chunks
			.into_iter()
			.map(|c| {
				let c = DBChunk::from(c);
				let users = c.access_users();
				let id = c.chunk().id.clone();
				let arc = Arc::new(RwLock::new(c));
				let weak = Arc::downgrade(&arc);
				users.iter().for_each(|u| {
					by_owner
						.entry(u.to_owned())
						.and_modify(|v| {
							if !v.iter().any(|v| v.ptr_eq(&weak)) {
								v.push(weak.clone());
							};
						})
						.or_insert_with(|| vec![weak.clone()]);
				});

				(id, arc)
			})
			.collect();

		let mut db = Self { chunks, by_owner };
		db.link_all().unwrap();
		db
	}
}
/**
 * From a reference because we're saving backups all the time, and it's easier to clone the underlying data
 */
impl From<&DB> for DBData {
	fn from(db: &DB) -> Self {
		DBData {
			chunks: db.chunks.values().map(|v| v.read().unwrap().chunk().clone()).collect(),
		}
	}
}

impl Serialize for DB {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		DBData::from(self).serialize(serializer)
	}
}
impl<'de> Deserialize<'de> for DB {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		DBData::deserialize(deserializer).map(Self::from)
	}
}
