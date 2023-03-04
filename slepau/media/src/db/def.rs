use super::{FileMeta, Media, MediaId, Task, Version, VersionString, DB};
use common::utils::{get_hash, LockedAtomic};
use common::utils::{DbError, CACHE_FOLDER};
use log::info;
use media::MEDIA_FOLDER;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
	collections::{hash_map::DefaultHasher, HashMap, HashSet},
	hash::{Hash, Hasher},
	io::{BufWriter, Cursor},
	sync::{Arc, RwLock},
};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

impl DB {
	pub fn new_id(&self) -> MediaId {
		let mut id;
		let mut i = 0;
		loop {
			id = Default::default();
			if !self.media.contains_key(&id) {
				break;
			}
			if i > 10 {
				panic!("ID clashing too much");
			}
			i += 1;
		}
		id
	}
	fn _tick(&mut self, v: &LockedAtomic<Media>) {
		let m = v.read().unwrap();
		m.versions.iter().for_each(|(v, info)| {
			if info.is_none() {
				// Push a task if cache for this
				self.task_queue.push_back(Task {
					priority: 0,
					id: m.id,
					version: v.clone(),
				})
			}
		})
	}
	pub fn tick_all(&mut self) {
		let all = self.media.values().cloned().collect::<Vec<_>>();
		all.iter().for_each(|m| self._tick(m));
	}
	pub fn get(&self, id: MediaId) -> Option<LockedAtomic<Media>> {
		self.media.get(&id).map(|v| v.to_owned())
	}
	pub fn add(&mut self, mut media: Media, owner: String) -> LockedAtomic<Media> {
		let id = media.id;
		// Extend versions with db init.
		if media.versions.is_empty() {
			media.versions.extend(
				self
					.initial_versions
					.iter()
					.filter(|(k, _)| media.meta._type.starts_with(*k))
					.map(|(_, v)| v)
					.flatten()
					.map(|v| (v.to_owned(), None)),
			);
		}
		let v = LockedAtomic::new(RwLock::new(media));
		self.media.entry(id).or_insert_with(|| v.clone());
		let weak = Arc::downgrade(&v);
		// self
		// 	.by_owner
		// 	.entry(owner)
		// 	.and_modify(|v| {
		// 		if !v.iter().any(|v| v.ptr_eq(&weak)) {
		// 			v.push(weak.clone());
		// 		};
		// 	})
		// 	.or_insert_with(|| vec![weak.clone()]);
		self
			.by_owner
			.entry(owner)
			.and_modify(|v| {
				v.insert(id);
			})
			.or_insert([id].into());

		self._tick(&v);
		v
	}
}

pub fn load_existing(db: LockedAtomic<DB>) {
	let path = std::path::Path::new(MEDIA_FOLDER.as_str());
	if let Ok(entries) = std::fs::read_dir(path) {
		for entry in entries {
			if let Ok(entry) = entry {
				let id = MediaId::from_quint(entry.file_name().to_str().unwrap()).unwrap();
				// Only add if we can't find it in the DB;
				if db.read().unwrap().get(id).is_none() {
					if let Ok(value) = std::fs::read(entry.path()) {
						let media = Media {
							id,
							meta: (&value).into(),
							..Default::default()
						};
						db.write().unwrap().add(media, "rubend".into());
					}
				}
			}
		}
	}
}

/**
 * DB data that will acutally get stored on disk
 */
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DBData {
	initial_versions: HashMap<String, HashSet<VersionString>>,
	media: Vec<Media>,
	by_owner: HashMap<String, HashSet<MediaId>>,
}

/**
 * Creates a base implementation of RAM data from what was saved
 */
impl From<DBData> for DB {
	fn from(data: DBData) -> Self {
		let media: HashMap<MediaId, LockedAtomic<Media>> = data
			.media
			.into_iter()
			.map(|c| {
				let id = c.id.clone();
				let arc = Arc::new(RwLock::new(c));
				(id, arc)
			})
			.collect();

		// let by_owner: HashMap<String, Vec<LockedWeak<Media>>> = data
		// 	.by_owner
		// 	.into_iter()
		// 	.map(|(owner, ids)| {
		// 		(
		// 			owner,
		// 			ids
		// 				.into_iter()
		// 				.filter_map(|id| media.get(&id).map(|m| Arc::downgrade(m)))
		// 				.collect(),
		// 		)
		// 	})
		// 	.collect();
		// let by_owner = data.by_owner.into_iter().map(|(k,v)| (k,HashSet::from_iter(v))).collect();

		let mut db = Self {
			initial_versions: data.initial_versions,
			media,
			by_owner: data.by_owner,
			..Default::default()
		};
		db.tick_all();
		db
	}
}
/**
 * From a reference because we're saving backups all the time, and it's easier to clone the underlying data
 */
impl From<&DB> for DBData {
	fn from(db: &DB) -> Self {
		Self {
			media: db.media.values().map(|v| v.read().unwrap().clone()).collect(),
			// by_owner: db
			// 	.by_owner
			// 	.iter()
			// 	.map(|v| {
			// 		(
			// 			v.0.to_owned(),
			// 			v.1
			// 				.iter()
			// 				.filter_map(|v| v.upgrade().map(|v| v.read().unwrap().id))
			// 				.collect(),
			// 		)
			// 	})
			// 	.collect(),
			by_owner: db.by_owner.clone(),
			initial_versions: db.initial_versions.clone(),
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
