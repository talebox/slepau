use super::{
	task::{Task, TaskCriteria},
	version::{Version, VersionReference, VersionString},
	Media, MediaId, DB,
};
use common::utils::{get_secs, DbError, LockedAtomic, CACHE_FOLDER};
use media::MEDIA_FOLDER;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
	collections::{HashMap, HashSet},
	path::PathBuf,
	sync::{Arc, RwLock},
};

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
	pub fn version_path(&self, _ref: &VersionReference) -> Option<PathBuf> {
		let m = self.media.get(&_ref.id)?.read().unwrap();
		let mut version = _ref.version.to_version().unwrap();
		// Media + Version => Path
		let to_path = |version: &Version| -> Option<PathBuf> {
			if json!(version).as_object().unwrap().len() == 0 {
				return Some(std::path::Path::new(MEDIA_FOLDER.as_str()).join(m.id.to_string()));
			}
			let version_string = version.into();
			m.versions.get(&version_string).map(|_| {
				std::path::Path::new(CACHE_FOLDER.as_str()).join(VersionReference::from((m.id, version_string)).filename_out())
			})
		};

		// Try getting path
		//
		// if unsuccessful AND type same as original type
		// then remove type from version and try again.
		to_path(&version).or_else(|| {
			if version._type.as_ref() == Some(&m.meta._type) {
				version._type = None;
				to_path(&version)
			} else {
				None
			}
		})
	}
	fn _tick(&mut self, media: &LockedAtomic<Media>) {
		let id = media.read().unwrap().id;
		let initial_versions = self.initial_versions.clone();

		initial_versions.iter().for_each(|(criteria, versions)| {
			if criteria.matches(&media.read().unwrap()) {
				versions.iter().for_each(|version| {
					// If we can't find a path to the version
					if self.version_path(&(id, version.clone()).into()).is_none() {
						let _ref = (id, version.clone()).into();
						// If you can't find a task with that version reference
						if self.task_queue.iter().find(|v| v._ref == _ref).is_none() {
							// Schedule a task for it
							self.task_queue.push_front((0, _ref).into())
						}
					}
				})
			}
		})
	}
	pub fn tick_all(&mut self) {
		self.task_queue.clear();
		let all = self.media.values().cloned().collect::<Vec<_>>();
		all.iter().for_each(|m| self._tick(m));
		self
			.task_queue
			.make_contiguous()
			.sort_by_key(|t| -(t.priority as isize));
	}
	pub fn get(&self, id: MediaId) -> Option<LockedAtomic<Media>> {
		self.media.get(&id).map(|v| v.to_owned())
	}
	pub fn get_all(&self) -> Vec<LockedAtomic<Media>> {
		self.media.values().cloned().collect()
	}
	pub fn add(&mut self, mut media: Media, owner: String) -> LockedAtomic<Media> {
		self
			.by_owner
			.entry(owner)
			.and_modify(|v| {
				v.insert(media.id);
			})
			.or_insert([media.id].into());

		if let Some(media) = self.media.get(&media.id) {
			media.to_owned()
		} else {
			media.created = get_secs();
			let id = media.id;
			let v = LockedAtomic::new(RwLock::new(media));
			self.media.insert(id, v.clone());
			self._tick(&v);
			v
		}
	}
	pub fn del(&mut self, id: MediaId) -> Result<LockedAtomic<Media>, DbError> {
		self.task_queue.retain(|task| task._ref.id != id);
		self.by_owner.iter_mut().for_each(|(_, ids)| {
			ids.remove(&id);
		});

		self.media.remove(&id).ok_or(DbError::NotFound)
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
						db.write().unwrap().add((id, &value).into(), "rubend".into());
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
	allow_public_post: bool,
	initial_versions: HashMap<TaskCriteria, HashSet<VersionString>>,
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

		let mut db = Self {
			allow_public_post: data.allow_public_post,
			initial_versions: data.initial_versions,
			media,
			by_owner: data.by_owner,
			task_queue: Default::default(),
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
			allow_public_post: db.allow_public_post,
			media: db.media.values().map(|v| v.read().unwrap().clone()).collect(),
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
