use super::{
	task::{convert::version_mapping, Task, TaskCriteria, TaskQuery},
	version::{VersionReference, VersionString},
	Media, MediaId, MediaStats, DB,
};
use common::utils::{get_secs, DbError, LockedAtomic};
use log::info;
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

	/// Tries to fetch version path, else returns a normalifzed VersionReference that has to be queued for a path to be found.
	pub fn version_path(&self, mut _ref: VersionReference) -> Result<PathBuf, Result<VersionReference, DbError>> {
		let media = self
			.get(_ref.id)
			.expect(format!("The id {} to be valid", _ref.id).as_str());

		let media = media.read().unwrap();

		let mut version = _ref.version.to_version().unwrap();
		version = version_mapping(&media.meta, version);

		if json!(version).as_object().unwrap().len() == 0 {
			return Ok(VersionReference::to_path_in(media.id));
		}
		let version_string: VersionString = (&version).into();

		let _ref = VersionReference::from((media.id, version_string.clone()));

		media
			.versions
			.get(&version_string)
			// Return Some(_ref) if you couldn't find the thing
			.ok_or(Ok(_ref.clone()))
			// Or if you found it, return an Err(None) to indicate that you found, but no good. or an Ok()
			.and_then(|v| {
				if let Some(err) = v.error.clone() {
					Err(Err(err.into()))
				} else {
					Ok(_ref.path_out())
				}
			})
	}
	/// Queues default version if it hasn't been built, returning the Path to the original.
	/// Or returns the path to the default version. This is guranteed to always return a path.
	pub fn default_version_path(&mut self, id: MediaId) -> PathBuf {
		// Check if the media exists
		if let Some(media) = self.get(id) {
			let default_version;
			{
				let media = media.read().unwrap();
				default_version = self.default_version.iter().find_map(|(criteria, version)| {
					if criteria.matches(&media) {
						Some(version.to_owned())
					} else {
						None
					}
				})
			}
			// Check if there's any default version for this media
			if let Some(default_version) = default_version {
				// Check if default version exists.
				match self.version_path((id, default_version.to_owned()).into()) {
					Ok(path) => {
						// Version exists
						return path;
					}
					Err(_ref) => {
						match _ref {
							Ok(_ref) => {
								// Version doesn't exist, just quee a task for it
								self.queue((0, _ref).into());
							}
							_ => {}
						}
					}
				}
			}
		}

		VersionReference::from((id, "".into())).path_in()
	}

	pub fn queue(&mut self, task: Task) {
		// if cfg!(debug_assertions) {
		// 	info!("Trying to queue {}", task._ref);
		// }
		// If you can't find a task with that version reference
		match self.task_queue.iter_mut().find(|v| v._ref == task._ref) {
			Some(_task) => {
				info!("Merged task {}.", task._ref);
				_task.priority = std::cmp::max(_task.priority, task.priority);
				_task.callbacks.extend(task.callbacks)
			}
			None => {
				info!("Scheduled task {}.", task._ref);
				// Schedule a task for it
				if task.priority > 0 {
					self.task_queue.push_front(task);
				} else {
					self.task_queue.push_back(task);
				}
				self
					.task_queue
					.make_contiguous()
					.sort_by_key(|t| -(t.priority as isize));
			}
		}
	}

	fn _tick(&mut self, media: &LockedAtomic<Media>) {
		let id = media.read().unwrap().id;
		let initial_versions = self.initial_versions.clone();

		initial_versions.iter().for_each(|(criteria, queries)| {
			if criteria.matches(&media.read().unwrap()) {
				queries.iter().for_each(|query| {
					// If we can't find a path to the version
					if let Err(Ok(_ref)) = self.version_path((id, query.version.clone()).into()) {
						// let _ref = (id, query.version.clone()).into();
						self.queue((0, _ref).into())
					}
				})
			}
		})
	}
	pub fn tick_all(&mut self) {
		self.task_queue.clear();
		let all = self.media.values().cloned().collect::<Vec<_>>();
		all.iter().for_each(|m| self._tick(m));
	}
	pub fn get(&self, id: MediaId) -> Option<LockedAtomic<Media>> {
		self.media.get(&id).map(|v| v.to_owned())
	}
	pub fn get_all(&self, user: &str) -> Vec<LockedAtomic<Media>> {
		self
			.by_owner
			.get(user)
			.map(|v| v.iter().filter_map(|v| v.upgrade()).collect())
			.unwrap_or_default()
	}
	pub fn user_stats(&self, user: &str) -> MediaStats {
		self
			.by_owner
			.get(user)
			.map(|medias| MediaStats::from_iter(medias))
			.unwrap_or_default()
	}
	pub fn add(&mut self, mut media: Media, owner: String) -> LockedAtomic<Media> {
		let _media;
		if let Some(media) = self.media.get(&media.id) {
			_media = media.to_owned()
		} else {
			media.created = get_secs();
			let id = media.id;
			_media = LockedAtomic::new(RwLock::new(media));
			self.media.insert(id, _media.clone());
			self._tick(&_media);
		}

		let _media_weak = Arc::downgrade(&_media);
		self
			.by_owner
			.entry(owner)
			.and_modify(|medias| {
				medias.push(_media_weak.clone());
			})
			.or_insert([_media_weak].into());

		_media
	}
	pub fn del(&mut self, id: MediaId, user: &str) -> Result<LockedAtomic<Media>, DbError> {
		// Get media
		let media = self.get(id).ok_or(DbError::NotFound)?;
		let media_weak = Arc::downgrade(&media);
		// Check that media belongs to owner
		self
			.by_owner
			.get(user)
			.map(|u| u.iter().any(|v| v.ptr_eq(&media_weak)))
			.ok_or(DbError::AuthError)?;

		// Remove all tasks for this media.
		self.task_queue.retain(|task| task._ref.id != id);
		// Remove this media from all owners.
		self.by_owner.iter_mut().for_each(|(_, medias)| {
			medias.retain(|v| !v.ptr_eq(&media_weak));
		});

		Ok(self.media.remove(&id).expect("Media has to exist, we just used it."))
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
	initial_versions: HashMap<TaskCriteria, Vec<TaskQuery>>,
	default_version: HashMap<TaskCriteria, VersionString>,
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
		let by_owner = data
			.by_owner
			.into_iter()
			.map(|(owner, ids)| {
				(
					owner,
					ids
						.into_iter()
						.filter_map(|id| media.get(&id).map(|v| Arc::downgrade(v)))
						.collect(),
				)
			})
			.collect();

		let mut db = Self {
			allow_public_post: data.allow_public_post,
			initial_versions: data.initial_versions,
			default_version: data.default_version,
			media,
			by_owner,
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
			// by_owner: db.by_owner.clone(),
			by_owner: db
				.by_owner
				.iter()
				.map(|(owner, medias)| {
					(
						owner.to_owned(),
						medias
							.iter()
							.filter_map(|v| v.upgrade().map(|v| v.read().unwrap().id.clone()))
							.collect(),
					)
				})
				.collect(),
			initial_versions: db.initial_versions.clone(),
			default_version: db.default_version.clone(),
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
