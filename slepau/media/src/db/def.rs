use common::utils::{LockedAtomic, LockedWeak};
use log::info;
use media::MEDIA_FOLDER;
use serde::{Deserialize, Serialize};
use std::{
	collections::{hash_map::DefaultHasher, HashMap},
	hash::{Hash, Hasher},
	io::{BufWriter, Cursor},
	sync::{Arc, RwLock},
};
use tokio::{select, sync::watch};

pub fn get_hash<T: Hash>(v: &T) -> u64 {
	let mut hasher = DefaultHasher::new();
	v.hash(&mut hasher);
	hasher.finish().into()
}
// one possible implementation of walking a directory only visiting files
fn visit_dirs(dir: &std::path::Path, cb: &dyn Fn(&std::fs::DirEntry)) -> std::io::Result<()> {
	if dir.is_dir() {
		for entry in std::fs::read_dir(dir)? {
			let entry = entry?;
			let path = entry.path();
			if path.is_dir() {
				visit_dirs(&path, cb)?;
			} else {
				cb(&entry);
			}
		}
	}
	Ok(())
}

use crate::db::CONVERSION_VERSION;

use super::{Media, MediaId, DB};

impl DB {
	pub fn get(&self, id: MediaId) -> Option<LockedAtomic<Media>> {
		self.media.get(&id).map(|v| v.to_owned())
	}
	pub fn add(&mut self, value: Media, owner: String) -> LockedAtomic<Media> {
		let id = value.id;
		let v = LockedAtomic::new(RwLock::new(value));

		self.media.entry(id).or_insert_with(|| v.clone());
		let weak = Arc::downgrade(&v);
		self
			.by_owner
			.entry(owner)
			.and_modify(|v| {
				if !v.iter().any(|v| v.ptr_eq(&weak)) {
					v.push(weak.clone());
				};
			})
			.or_insert_with(|| vec![weak.clone()]);

		self.conversion_queue.push_back(id);
		v
	}
	pub fn set_bytes(&mut self, value: Vec<u8>, owner: String) -> LockedAtomic<Media> {
		// Calculate hash
		let id: MediaId = get_hash(&value).into();
		let _type = infer::get(&value);
		let matcher_type = _type.map(|v| v.matcher_type()).unwrap_or(infer::MatcherType::Custom);

		let media = Media {
			id,
			name: Default::default(),
			size: value.len(),
			_type: matcher_type,
			conversion: Default::default(),
		};

		self.add(media, owner)
	}
}

pub fn load_existing(db: LockedAtomic<DB>) {
	let path = std::path::Path::new(MEDIA_FOLDER.as_str());
	if let Ok(entries) = std::fs::read_dir(path) {
		for entry in entries {
			if let Ok(entry) = entry {
				if let Ok(value) = std::fs::read(entry.path()) {
					// info!("Sending {} bytes file", value.len());
					let id: MediaId =
						MediaId::from_quint(entry.file_name().to_str().unwrap()).unwrap_or_else(|_| get_hash(&value).into());
					let _type = infer::get(&value);
					let matcher_type = _type.map(|v| v.matcher_type()).unwrap_or(infer::MatcherType::Custom);

					let media = Media {
						id,
						name: Default::default(),
						size: value.len(),
						_type: matcher_type,
						conversion: Default::default(),
					};

					db.write().unwrap().add(media, "rubend".into());
				}
			}
		}
	}
}

/// Does conversion, this is the function spawned
/// that actually does the conversion and updates accordingly
fn do_convert(db: LockedAtomic<DB>, id: MediaId) -> std::io::Result<()> {
	let should_convert;
	// We first just try to work with what's on RAM
	{
		let mut db = db.write().unwrap();
		let media = db.media.get(&id);
		let sc = media.map(|m| {
			let m = m.read().unwrap();
			// info!("{m:?}");
			m.conversion.version != CONVERSION_VERSION
		});
		should_convert = sc.unwrap_or(false);
		if should_convert {
			db.conversion_current.push(id);
		}
	}
	// Then we read the file and see if it conversion is needed.
	if should_convert {
		let path = std::path::Path::new(MEDIA_FOLDER.as_str()).join(id.to_quint());
		let mut file = std::fs::read(&path)?;
		let prev_file_size = file.len();

		let _type = infer::get(&file);
		let matcher_type = _type.map(|v| v.matcher_type()).unwrap_or(infer::MatcherType::Custom);
		let mime_type = _type.map(|v| v.mime_type()).unwrap_or_default();

		let mut convert_to = Default::default();
		{
			match matcher_type {
				infer::MatcherType::Image => {
					convert_to = "image/webp";
				}
				_ => {}
			}
		}

		if mime_type != convert_to {
			info!("Converting {id}");
			let now = std::time::Instant::now();
			// Figure out type and convert
			{
				if convert_to == "image/webp" {
					let img = image::load_from_memory(&file).unwrap();
					let mut _out = BufWriter::new(Cursor::new(vec![]));
					// info!("Converting image w:{},h:{} to .avif", img.width(), img.height());
					img.write_to(&mut _out, image::ImageOutputFormat::WebP).unwrap();
					// info!("Finished conversion of w:{},h:{}", img.width(), img.height());
					file = _out.into_inner().unwrap().into_inner().into();
				}
			}
			let delay = (std::time::Instant::now() - now).as_secs_f32();
			let size_reduction = (1. - (file.len() as f32 / prev_file_size as f32)) * 100.;
			info!("Done Converting {id}");

			let size = file.len();
			let changes = prev_file_size != size;
			if changes {
				std::fs::write(&path, file)?;
			}
			{
				let mut db = db.write().unwrap();
				db.conversion_current.retain(|v| *v != id);
				let mut media = db.media.get(&id).unwrap().write().unwrap();
				media.conversion.version = CONVERSION_VERSION;
				if changes {
					media.size = size;
					media.conversion.time = delay;
					media.conversion.size_reduction = size_reduction;
					media.conversion.format = convert_to.into();
					info!("Delay {delay}, size_reduction {size_reduction} for {id}");
				}
			}
		}
	}

	Ok(())
}

pub async fn conversion_service(db: LockedAtomic<DB>, mut shutdown_rx: watch::Receiver<()>) {
	let mut handles = tokio::task::JoinSet::new();
	let cpus = num_cpus::get();
	loop {
		loop {
			if handles.len() >= cpus {
				break;
			}
			let id;
			{
				let mut db = db.write().unwrap();
				id = db.conversion_queue.pop_front();
			}
			if let Some(id) = id {
				let db = db.clone();
				handles.spawn(tokio::task::spawn_blocking(move || do_convert(db, id)));
			} else {
				break;
			}
		}

		if handles.is_empty() {
			tokio::select! {
				_ = shutdown_rx.changed() => {
					break;
				}
				_ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
			}
		} else {
			tokio::select! {
				_ = shutdown_rx.changed() => {
					break;
				}
				_ = handles.join_next() => {}
			}
		}
	}

	info!("Aborting all handles.");
}

/**
 * DB data that will acutally get stored on disk
 */
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DBData {
	media: Vec<Media>,
	by_owner: HashMap<String, Vec<MediaId>>,
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

		let by_owner: HashMap<String, Vec<LockedWeak<Media>>> = data
			.by_owner
			.into_iter()
			.map(|(owner, ids)| {
				(
					owner,
					ids
						.into_iter()
						.filter_map(|id| media.get(&id).map(|m| Arc::downgrade(m)))
						.collect(),
				)
			})
			.collect();

		let mut db = Self {
			media,
			by_owner,
			..Default::default()
		};
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
			by_owner: db
				.by_owner
				.iter()
				.map(|v| {
					(
						v.0.to_owned(),
						v.1
							.iter()
							.filter_map(|v| v.upgrade().map(|v| v.read().unwrap().id))
							.collect(),
					)
				})
				.collect(),
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
