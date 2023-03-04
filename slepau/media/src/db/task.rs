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

/// Does conversion, this is the function spawned
/// that actually does the conversion and updates accordingly
fn do_convert(task: Task) -> Result<(Task, Vec<u8>), DbError> {
	let path = std::path::Path::new(MEDIA_FOLDER.as_str()).join(task.id.to_quint());
	let data = std::fs::read(&path).map_err(|e| DbError::NotFound)?;
	let version: Version = (&task.version).into();

	if let Some(_type) = version._type {
		if _type == "image/webp" {
			let img = image::load_from_memory(&data).unwrap();
			let mut _out = BufWriter::new(Cursor::new(vec![]));
			img.write_to(&mut _out, image::ImageOutputFormat::WebP).unwrap();
			return Ok((task, _out.into_inner().unwrap().into_inner().into()));
		}
	} else {
		return Err(DbError::Custom("Type required in Task."));
	}

	Err(DbError::Custom("Error executing task."))

	// let should_convert;
	// // We first just try to work with what's on RAM
	// {
	// 	let mut db = db.write().unwrap();
	// 	let media = db.media.get(&id);
	// 	let sc = media.map(|m| {
	// 		let m = m.read().unwrap();
	// 		// info!("{m:?}");
	// 		m.conversion.version != CONVERSION_VERSION
	// 	});
	// 	should_convert = sc.unwrap_or(false);
	// 	if should_convert {
	// 		db.conversion_current.push(id);
	// 	}
	// }
	// // Then we read the file and see if it conversion is needed.
	// if should_convert {
	// 	let path = std::path::Path::new(MEDIA_FOLDER.as_str()).join(id.to_quint());
	// 	let mut file = std::fs::read(&path)?;
	// 	let prev_file_size = file.len();

	// 	let mut convert_to = Default::default();
	// 	{
	// 		match matcher_type {
	// 			infer::MatcherType::Image => {
	// 				convert_to = "image/webp";
	// 			}
	// 			_ => {}
	// 		}
	// 	}

	// 	if mime_type != convert_to {
	// 		info!("Converting {id}");
	// 		let now = std::time::Instant::now();
	// 		// Figure out type and convert
	// 		{
	// 			if convert_to == "image/webp" {
	// let img = image::load_from_memory(&file).unwrap();
	// let mut _out = BufWriter::new(Cursor::new(vec![]));
	// // info!("Converting image w:{},h:{} to .avif", img.width(), img.height());
	// img.write_to(&mut _out, image::ImageOutputFormat::WebP).unwrap();
	// // info!("Finished conversion of w:{},h:{}", img.width(), img.height());
	// file = _out.into_inner().unwrap().into_inner().into();
	// 			}
	// 		}
	// 		let delay = (std::time::Instant::now() - now).as_secs_f32();
	// 		let size_reduction = (1. - (file.len() as f32 / prev_file_size as f32)) * 100.;
	// 		info!("Done Converting {id}");

	// 		let size = file.len();
	// 		let changes = prev_file_size != size;
	// 		if changes {
	// 			std::fs::write(&path, file)?;
	// 		}
	// 		{
	// 			let mut db = db.write().unwrap();
	// 			db.conversion_current.retain(|v| *v != id);
	// 			let mut media = db.media.get(&id).unwrap().write().unwrap();
	// 			media.conversion.version = CONVERSION_VERSION;
	// 			if changes {
	// 				media.size = size;
	// 				media.conversion.time = delay;
	// 				media.conversion.size_reduction = size_reduction;
	// 				media.conversion.format = convert_to.into();
	// 				info!("Delay {delay}, size_reduction {size_reduction} for {id}");
	// 			}
	// 		}
	// 	}
	// }

	// Ok(())
}

pub async fn conversion_service(
	db: LockedAtomic<DB>,
	mut shutdown_rx: watch::Receiver<()>,
	mut media_tx: watch::Sender<MediaId>,
	mut task_rx: mpsc::Receiver<(Task, Option<oneshot::Sender<()>>)>,
) {
	let mut handles = tokio::task::JoinSet::new();
	let cpus = num_cpus::get();
	loop {
		loop {
			if handles.len() >= cpus {
				break;
			}
			let task;
			{
				let mut db = db.write().unwrap();
				task = db.task_queue.pop_front();
			}
			if let Some(task) = task {
				handles.spawn(tokio::task::spawn_blocking(move || (do_convert(task), None)));
			} else {
				break;
			}
		}

		let wait_conversion = async {
			if handles.len() > 0 {
				let r = handles.join_next().await.unwrap().unwrap().unwrap();

				if let (Ok((task, data)), channel) = r {
					let out_folder = std::path::Path::new(CACHE_FOLDER.as_str());
					if !out_folder.exists() {
						tokio::fs::create_dir(out_folder).await.unwrap();
					}
					let out_path = out_folder.join(MediaId::from(get_hash(&task)).to_quint());

					info!("Writing to {out_path:?}");
					tokio::fs::write(out_path, data).await.unwrap();
				} else {
					log::error!("Conversion failed {r:?}");
				}
			} else {
				tokio::time::sleep(std::time::Duration::from_secs(10)).await;
			}
		};

		tokio::select! {
			_ = shutdown_rx.changed() => {
				break;
			}
			_ = wait_conversion => {}
			Some((task,channel)) = task_rx.recv() => {
				handles.spawn(tokio::task::spawn_blocking(move || (do_convert(task), channel)));
			}
		}
	}

	info!("Aborting all handles.");
}

impl From<&Vec<u8>> for FileMeta {
	fn from(value: &Vec<u8>) -> Self {
		let _type = infer::get(value);
		let mime_type = _type.map(|v| v.mime_type()).unwrap_or_default();
		Self {
			hash: get_hash(value).into(),
			size: value.len(),
			_type: mime_type.into(),
		}
	}
}

impl From<&Vec<u8>> for Media {
	fn from(value: &Vec<u8>) -> Self {
		Self {
			meta: value.into(),
			..Default::default()
		}
	}
}

impl From<&Version> for VersionString {
	fn from(value: &Version) -> Self {
		Self(
			serde_json::to_value(value)
				.unwrap()
				.as_object()
				.unwrap()
				.iter()
				.map(|(k, v)| format!("{k}={v}"))
				.collect::<Vec<_>>()
				.join("&")
				.to_string(),
		)
	}
}
impl From<&str> for VersionString {
	fn from(value: &str) -> Self {
		Self(value.into())
	}
}
impl From<&VersionString> for Version {
	fn from(value: &VersionString) -> Self {
		let value = value
			.0
			.split("&")
			.map(|v| {
				let a = v.split("=").collect::<Vec<_>>();
				if a.len() != 2 {
					panic!("Not valid key=value in Version parsing.");
				}
				(
					a[0].to_string(),
					serde_json::from_str::<Value>(a[1])
						.unwrap_or_else(|_| serde_json::from_str::<Value>(&format!("\"{}\"", a[1])).unwrap()),
				)
			})
			.collect::<HashMap<_, _>>();
		serde_json::from_value(json!(value)).unwrap()
	}
}
impl Serialize for VersionString {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		self.0.serialize(serializer)
	}
}
impl<'de> Deserialize<'de> for VersionString {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Ok(Self(String::deserialize(deserializer)?))
	}
}

impl Hash for Version {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		VersionString::from(self).hash(state)
	}
}
impl Hash for Task {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.id.hash(state);
		self.version.hash(state);
	}
}
