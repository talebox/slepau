use super::version::{VersionReference, VersionString};
use super::{Media, DB};
use common::socket::{ResourceMessage, SocketMessage};
use common::utils::{DbError, LockedAtomic};
use log::info;
use serde::{Deserialize, Serialize};

use std::{
	hash::{Hash, Hasher},
	time::Instant,
};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

pub mod convert;

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TaskCriteria(String);
impl TaskCriteria {
	pub fn matches(&self, media: &Media) -> bool {
		media.meta._type.starts_with(&self.0)
	}
}

fn is_false(b: &bool) -> bool {
	!(*b)
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
/// Holds data that that defines task initialization parameters
pub struct TaskQuery {
	pub version: VersionString,
	#[serde(skip_serializing_if = "is_false")]
	pub replace: bool,
}

pub type TaskOneshot = oneshot::Sender<Result<(), DbError>>;

#[derive(Default, Debug)]
pub struct Task {
	pub priority: usize,
	pub _ref: VersionReference,
	pub callbacks: Vec<TaskOneshot>,
	pub started: Option<Instant>,
	pub replace: bool,
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
		{
			let mut db = db.write().unwrap();
			let mut spawned = false;
			// This loop fills the JoinSet with tasks.
			loop {
				if handles.len() >= cpus {
					break;
				}

				if let Some(task) = db.task_queue.iter_mut().find(|v| v.started.is_none()) {
					task.started = Some(Instant::now());
					let _ref = task._ref.to_owned();
					handles.spawn(tokio::task::spawn_blocking(move || convert::do_convert(_ref)));
					info!("Spawned task {}", task._ref);
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
			r = handles.join_next(), if !handles.is_empty() => {
				match r.unwrap().flatten() {
					Ok(v) => {

						let _ref;
						let meta;
						let out_path;
						let err;
						match v {
							Ok((__ref, _meta, _out_path)) => {
								log::info!("Task {} success!", __ref);
								_ref = __ref;
								meta = Some(_meta);
								out_path = Some(_out_path);
								err = None;
							}
							Err((__ref, _err)) => {
								log::error!("Task {} failed: {_err:?}", __ref);
								_ref = __ref;
								meta = None;
								out_path = None;
								err = Some(_err);
							}
						}

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
									info.meta = meta.unwrap_or_default();
									info.error = err.clone().map(|v| format!("{v:?}"));
									
									m.versions.insert(task._ref.version, info);
								}else {
									// Remove the file, most likely the entry was deleted.
									if let Some(out_path) = out_path {
										tokio::fs::remove_file(out_path).await.ok();
									}
								}
							}
							// Notify
							for callback in task.callbacks {
								if let Some(err) = err.clone() {
									callback.send(Err(err)).ok();
								}else{
									callback.send(Ok(())).ok();
								}
							}
							// Notify
							tx_resource.send(format!("media/{}", task._ref.id).as_str().into()).ok();
						}
					}
					Err(join_err) => {
						log::error!("Task failed: {join_err}")
					}
				}
			}
			Some(task) = task_rx.recv() => {
				db.write().unwrap().queue(task);
			}
			_ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {}
		}
	}

	info!("Aborting all handles.");
}
