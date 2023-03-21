/// Common data management functionality for the Slepau, initialization, backups etc...
use hyper::StatusCode;
use log::{error, info, trace};
use serde::{de::DeserializeOwned, Serialize};
use std::fs;

use crate::utils::{DB_INIT, DB_PATH};

pub mod backup;
pub mod magic_bean;

pub async fn init<T: DeserializeOwned + Default>() -> T {
	fn failover<T: Default>(path: &str) -> T {
		info!("Reading {} failed, initializing empty DB", path);
		T::default()
	}

	// If db_init present, then attempt to connect to it's URL and initialize from it
	match DB_INIT.as_ref() {
		Some(db_init) => {
			trace!("Fetching {}", db_init);
			match reqwest::get(format!("{db_init}/api/mirror/{}", magic_bean::MAGIC_BEAN)).await {
				Ok(v) => {
					if v.status() != StatusCode::OK {
						return failover(db_init);
					}
					serde_json::from_slice::<T>(&v.bytes().await.unwrap()).unwrap()
				}
				_ => failover(db_init),
			}
		}
		None => match DB_PATH.clone() {
			Some(db_path) => match fs::read_to_string(&db_path) {
				Ok(db_json) => {
					let db_in = serde_json::from_str::<T>(db_json.as_str()).unwrap();

					info!("Read {}", &db_path);

					db_in
				}
				_ => failover(&db_path),
			},
			None => failover("None"),
		},
	}
}
pub fn save<T: Serialize>(db: &T) {
	if let Some(db_path) = DB_PATH.clone() {
		#[cfg(debug_assertions)]
		let data = serde_json::to_string_pretty(db).unwrap();

		#[cfg(not(debug_assertions))]
		let data = serde_json::to_string(db).unwrap();

		match fs::write(&db_path, &data) {
			Ok(()) => info!("Saved on {}", db_path),
			Err(e) => {
				error!("Error saving to path {}: {e}", &db_path);
				let backup_path = "db.backup.json".to_string();
				match fs::write(&backup_path, &data) {
					Ok(()) => info!("Saved db on backup {backup_path}"),
					Err(e) => error!("Error saving to backup path {backup_path}: {e}"),
				}
			}
		};
	}
}
