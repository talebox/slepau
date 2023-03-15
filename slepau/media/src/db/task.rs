use common::socket::{ResourceMessage, SocketMessage};
use common::utils::{DbError, LockedAtomic, CACHE_FOLDER};
use image::imageops::FilterType;
use image::{ImageFormat, ImageOutputFormat};
use log::info;
use media::MEDIA_FOLDER;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{
	hash::{Hash, Hasher},
	io::{BufWriter, Cursor},
	time::Instant,
};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use super::meta::FileMeta;
use super::version::{Max, Version, VersionReference};
use super::{Media, DB};

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskCriteria(String);
impl TaskCriteria {
	pub fn matches(&self, media: &Media) -> bool {
		media.meta._type.starts_with(&self.0)
	}
}

pub type TaskOneshot = oneshot::Sender<Result<(), DbError>>;

#[derive(Default, Debug)]
pub struct Task {
	pub priority: usize,
	pub _ref: VersionReference,
	pub callbacks: Vec<TaskOneshot>,
	pub started: Option<Instant>,
}
impl From<(usize, VersionReference)> for Task {
	fn from((priority, _ref): (usize, VersionReference)) -> Self {
		Self {
			priority,
			_ref,
			..Default::default()
		}
	}
}
impl From<(usize, VersionReference, TaskOneshot)> for Task {
	fn from((priority, _ref, callback): (usize, VersionReference, TaskOneshot)) -> Self {
		Self {
			priority,
			_ref,
			callbacks: vec![callback],
			..Default::default()
		}
	}
}
impl Hash for Task {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self._ref.hash(state);
	}
}

/// Does conversion, this is the function spawned
/// that actually does the conversion and updates accordingly
fn do_convert(_ref: VersionReference) -> Result<(VersionReference, FileMeta, PathBuf), (VersionReference, DbError)> {
	let out_folder = std::path::Path::new(CACHE_FOLDER.as_str());
	if !out_folder.exists() {
		std::fs::create_dir(out_folder).unwrap();
	}
	let path_in = std::path::Path::new(MEDIA_FOLDER.as_str()).join(_ref.filename_in());
	let path_out = out_folder.join(_ref.filename_out());

	// let data = std::fs::read(&path).map_err(|e| (_ref.clone(), DbError::NotFound))?;
	let meta = FileMeta::from_path(&path_in);
	let version: Version = (&_ref.version).into();

	if meta._type.starts_with("image") {
		// let mut format = image::guess_format(&data).unwrap();
		let mut format = ImageFormat::from_mime_type(meta._type.clone()).ok_or_else(|| {
			(
				_ref.clone(),
				DbError::from(format!("Unknown image type '{}'.", meta._type)),
			)
		})?;

		let mut img = image::load(
			std::io::BufReader::new(std::fs::File::open(path_in).ok().unwrap()),
			format.clone(),
		)
		.unwrap();

		if let Some(orientation) = meta.exif.and_then(|v| {
			v.to_exif()
				.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
				.cloned()
		}) {
			let v = orientation.value.get_uint(0).unwrap();
			if [2, 4].contains(&v) {
				img = img.fliph();
			} else if [5, 7].contains(&v) {
				img = img.flipv();
			}
			if [5, 6].contains(&v) {
				img = img.rotate90();
			}
			if [3, 4].contains(&v) {
				img = img.rotate180();
			}
			if [8, 7].contains(&v) {
				img = img.rotate270();
			}
		}

		if let Some(max) = version.max {
			let mut width = img.width() as f32;
			let mut height = img.height() as f32;

			match max {
				Max::Absolute(x, y) => {
					if let Some(x) = x {
						width = x as f32
					}
					if let Some(y) = y {
						height = y as f32
					}
				}
				Max::Area(area) => {
					let max_area = area as f32;
					let max_to_current = (max_area / (width * height)).sqrt();
					if max_to_current < 1. {
						width = width * max_to_current;
						height = height * max_to_current;
					}
				}
			}
			img = img.resize(width.round() as u32, height.round() as u32, FilterType::Triangle);
		}

		if let Some(_type) = version._type {
			format = ImageFormat::from_mime_type(_type.clone())
				.ok_or_else(|| (_ref.clone(), DbError::from(format!("Unknown image type '{}'.", _type))))?;
		}

		// let mut _out = BufWriter::new(Cursor::new(vec![]));

		// let format_out = ImageOutputFormat::from(format);
		

		img.save_with_format(path_out.clone(), format).unwrap();
		
		
		
		let meta = FileMeta::from_path(&path_out);
		return Ok((_ref, meta, path_out));
	} else {
		return Err((
			_ref,
			format!("Can't convert from unknown type '{}'.", meta._type).into(),
		));
	}
}

pub async fn conversion_service(
	db: LockedAtomic<DB>,
	mut shutdown_rx: watch::Receiver<()>,
	tx_resource: broadcast::Sender<ResourceMessage>,
	mut task_rx: mpsc::Receiver<Task>,
) {
	let mut handles = tokio::task::JoinSet::new();

	let cpus = num_cpus::get();
	let send_tasks = |db: &DB| {
		tx_resource
			.send(SocketMessage::from(("tasks", &db.tasks_len())).into())
			.ok();
	};
	loop {
		// This loop fills the JoinSet with tasks.
		{
			let mut db = db.write().unwrap();
			let mut spawned = false;
			loop {
				if handles.len() >= cpus {
					break;
				}

				if let Some(task) = db.task_queue.iter_mut().find(|v| v.started.is_none()) {
					task.started = Some(Instant::now());
					let _ref = task._ref.to_owned();
					handles.spawn(tokio::task::spawn_blocking(move || do_convert(_ref)));
					spawned = true;
				} else {
					break;
				}
			}
			if spawned {
				send_tasks(&db);
			}
		}

		tokio::select! {
			_ = shutdown_rx.changed() => {
				break;
			}
			r = handles.join_next(), if handles.len() > 0 => {

				match r.unwrap().flatten().unwrap() {
					Ok((_ref, meta, out_path)) => {
						let task;
						{
							let mut db = db.write().unwrap();
							task = db.task_queue.iter().position(|v| v._ref == _ref).and_then(|p| db.task_queue.remove(p));
							send_tasks(&db);
						}
						if let Some(task) = task {
							let time = Instant::now() - task.started.expect("This task should have started before finishing :|");

							// let meta: FileMeta = (&data).into();
							// tokio::fs::write(out_path.clone(), data).await.unwrap();
							{
								let m = db.read().unwrap().get(task._ref.id);
								if let Some(m) = m {
									let mut m = m.write().unwrap();
									// Only modify time/meta on versioninfo
									let mut info = m.versions.get(&task._ref.version).cloned().unwrap_or_default();
									info.time = time.as_secs_f32();
									info.meta = meta;

									m.versions.insert(task._ref.version, info);
								}else {
									// Remove the file, most likely the entry was deleted.
									tokio::fs::remove_file(out_path).await.ok();
								}
							}
							// Notify
							for callback in task.callbacks {
								callback.send(Ok(())).ok();
							}
							// Notify
							tx_resource.send(format!("media/{}", task._ref.id).as_str().into()).ok();
						}
					}
					Err((_ref, err)) => {
						let task;
						{
							let mut db = db.write().unwrap();
							task = db.task_queue.iter().position(|v| v._ref == _ref).and_then(|p| db.task_queue.remove(p));
						}
						if let Some(task) = task {
							for callback in task.callbacks {
								callback.send(Err(err.clone())).ok();
							}
						}
					}
				}
			}
			Some(task) = task_rx.recv() => {
				let mut db = db.write().unwrap();
				let _task = db.task_queue.iter_mut().find(|v| v._ref == task._ref);
				if let Some(_task) = _task {
					_task.priority = std::cmp::max(_task.priority, task.priority);
					_task.callbacks.extend(task.callbacks)
				}else{
					db.task_queue.push_front(task);
					// send_tasks(&db);
				}
			}
			_ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {}
		}
	}

	info!("Aborting all handles.");
}
