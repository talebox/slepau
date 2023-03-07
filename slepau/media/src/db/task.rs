use crate::db::VersionInfo;
use super::{FileMeta, Media, MediaId, Task, Version, VersionReference, VersionString, DB};
use common::socket::{ResourceMessage, SocketMessage};
use common::utils::{get_hash, LockedAtomic};
use common::utils::{DbError, CACHE_FOLDER};
use exif::Tag;
use image::imageops::FilterType;
use image::{ImageFormat, ImageOutputFormat};
use log::info;
use media::MEDIA_FOLDER;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::Display;
use std::io::{BufReader, Bytes};
use std::time::Instant;
use std::{
	collections::{hash_map::DefaultHasher, HashMap, HashSet},
	hash::{Hash, Hasher},
	io::{BufWriter, Cursor},
};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

/// Does conversion, this is the function spawned
/// that actually does the conversion and updates accordingly
fn do_convert(task: Task) -> Result<(Task, Vec<u8>), DbError> {
	let path = std::path::Path::new(MEDIA_FOLDER.as_str()).join(task._ref.id.to_quint());
	let data = std::fs::read(&path).map_err(|e| DbError::NotFound)?;

	let meta: FileMeta = (&data).into();
	let version: Version = (&task._ref.version).into();

	if meta._type.starts_with("image") {
		let mut format = image::guess_format(&data).unwrap();
		let mut img = image::load_from_memory_with_format(&data, format.clone()).unwrap();
		
		if let Some(orientation) = meta.exif.and_then(|v| v.to_exif().get_field(exif::Tag::Orientation, exif::In::PRIMARY).cloned()){
			let v = orientation.value.get_uint(0).unwrap();
			if [2,4].contains(&v) {
				img = img.fliph();
			}else if [5,7].contains(&v) {
				img = img.flipv();
			}
			if [5,6].contains(&v) {
				img = img.rotate90();
			}
			if [3,4].contains(&v) {
				img = img.rotate180();
			}
			if [8,7].contains(&v) {
				img = img.rotate270();
			}
		}

		if let Some(max) = version.max {
			let mut width = img.width() as f32;
			let mut height = img.height() as f32;
			let max = max as f32;
			let max_to_current = max / (width * height).sqrt();
			info!("Max to current {max_to_current}");
			if max_to_current < 1. {
				width = width * max_to_current;
				height = height * max_to_current;
			}
			info!("new width {width}, new height {height}");
			img = img.resize(width.round() as u32, height.round() as u32, FilterType::Triangle);
		}

		if let Some(_type) = version._type {
			format = ImageFormat::from_mime_type(_type.clone())
				.ok_or_else(|| DbError::from(format!("Unknown image type '{}'.", _type)))?;
		}

		let mut _out = BufWriter::new(Cursor::new(vec![]));

		let format_out = ImageOutputFormat::from(format);

		img.write_to(&mut _out, format_out).unwrap();
		return Ok((task, _out.into_inner().unwrap().into_inner().into()));
	} else {
		return Err(format!("Can't convert from unknown type '{}'.", meta._type).into());
	}

	Err("Error executing task.".into())
}

type TaskOneshot = oneshot::Sender<Result<(), DbError>>;
pub type TaskRequest = (Task, Option<TaskOneshot>);

pub async fn conversion_service(
	db: LockedAtomic<DB>,
	mut shutdown_rx: watch::Receiver<()>,
	mut tx_resource: broadcast::Sender<ResourceMessage>,
	mut task_rx: mpsc::Receiver<TaskRequest>,
) {
	let mut handles = tokio::task::JoinSet::new();
	let cpus = num_cpus::get();
	loop {
		// This loop fills the JoinSet with tasks.
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
				handles.spawn(tokio::task::spawn_blocking(move || {
					let start = Instant::now();
					let n: Option<TaskOneshot> = None;
					(do_convert(task), n, start)
				}));
			} else {
				break;
			}
		}

		tokio::select! {
			_ = shutdown_rx.changed() => {
				break;
			}
			r = handles.join_next(), if handles.len() > 0 => {
				if let (Ok((task, data)), channel, start) = r.unwrap().unwrap().unwrap() {
					let time = Instant::now() - start;

					let out_folder = std::path::Path::new(CACHE_FOLDER.as_str());
					if !out_folder.exists() {
						tokio::fs::create_dir(out_folder).await.unwrap();
					}
					let out_path = out_folder.join(task._ref.to_filename());

					info!("Writing {task:?} to {out_path:?}");
					let meta: FileMeta = (&data).into();
					tokio::fs::write(out_path, data).await.unwrap();
					{
						let m = db.write().unwrap().get(task._ref.id).unwrap();
						let mut m = m.write().unwrap();
						// Only modify time/meta on versioninfo
						let mut info = m.versions.get(&task._ref.version).cloned().unwrap_or_default();
						info.time = time.as_secs_f32();
						info.meta = meta;

						m.versions.insert(task._ref.version, info);
					}
					// Notify
					if let Some(channel) = channel {
						channel.send(Ok(())).ok();
					}
					// Notify
					tx_resource.send("media".into()).ok();
				}
			}
			Some((task,channel)) = task_rx.recv() => {
				handles.spawn(tokio::task::spawn_blocking(move || {
					let start = Instant::now();
					(do_convert(task), channel, start)
				}));
			}
			_ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {}
		}
	}

	info!("Aborting all handles.");
}

impl From<&Vec<u8>> for FileMeta {
	fn from(value: &Vec<u8>) -> Self {
		let _type = infer::get(value);
		let mime_type = _type.map(|v| v.mime_type()).unwrap_or_default();
		let extra = super::Exif::from_img(value);
		Self {
			hash: get_hash(value).into(),
			size: value.len(),
			_type: mime_type.into(),
			exif: extra,
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
impl From<(MediaId, &Vec<u8>)> for Media {
	fn from((id, value): (MediaId, &Vec<u8>)) -> Self {
		Self {
			id,
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
impl From<&VersionString> for Version {
	fn from(value: &VersionString) -> Self {
		value.to_version().unwrap()
	}
}

impl From<&str> for VersionString {
	fn from(value: &str) -> Self {
		Self::new(value).unwrap()
	}
}
impl VersionString {
	pub fn new(value: &str) -> Result<Self, DbError> {
		// &str -> VersionString
		let s = Self(value.into());
		// VersionString -> Version
		let s = s.to_version()?;
		// Version -> VersionString
		let s = Self::from(&s);
		Ok(s)
	}
	pub fn to_version(&self) -> Result<Version, DbError> {
		if self.0.is_empty() {
			return Ok(Default::default());
		}

		let value = self.0.split("&").map(|v| v.split("=").collect::<Vec<_>>());

		if value.clone().any(|v| v.len() != 2) {
			return Err("All records (separated by '&') to have exactly 1 key and 1 value separated by an '='.".into());
		}
		let value = value
			.map(|v| {
				let key = v[0];
				let value = v[1];
				(
					key.to_string(),
					serde_json::from_str::<Value>(value)
						.unwrap_or_else(|_| serde_json::from_str::<Value>(&format!("\"{}\"", value)).unwrap()),
				)
			})
			.collect::<HashMap<_, _>>();

		Ok(
			serde_json::from_value(json!(value))
				.map_err(|err| DbError::from(format!("Serde parsing Error: {err}").as_str()))?,
		)
	}
}

impl Display for VersionString {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}
impl<'de> Deserialize<'de> for VersionString {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Ok(Self::from(String::deserialize(deserializer)?.as_str()))
	}
}

// impl Hash for Version {
// 	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
// 		VersionString::from(self).hash(state)
// 	}
// }

// impl Display for Task {
// 	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
// 		write!(f, "{},{}", self.id, self.version)
// 	}
// }
impl Hash for Task {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self._ref.hash(state);
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn version_string() {
		assert_eq!(
			"type=\"img/test\"",
			VersionString::from("type=img/test").0,
			"Should parse strings without quotes correctly."
		);
		assert_eq!(
			"type=\"img/test\"&xm=123",
			VersionString::from("nothing=jeesh&xm=123&type=img/test").0,
			"Should only allow Version key and should reorder accordingly."
		);
	}
}
