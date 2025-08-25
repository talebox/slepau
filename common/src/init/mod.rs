/// Common data management functionality for the Slepau, initialization, backups etc...
use log::{error, info, trace};
use serde::{de::DeserializeOwned, Serialize};
use std::fs;


use crate::utils::{LockedAtomic, DB_INIT, DB_PATH};

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
			failover(db_init)
			// match reqwest::get(format!("{db_init}/api/mirror/{}", magic_bean::MAGIC_BEAN)).await {
			// 	Ok(v) => {
			// 		if v.status() != StatusCode::OK {
			// 			return failover(db_init);
			// 		}
			// 		serde_json::from_slice::<T>(&v.bytes().await.unwrap()).unwrap()
			// 	}
			// 	_ => failover(db_init),
			// }
		}
		None => {
			let db_path = DB_PATH.clone();
			match fs::read_to_string(&db_path) {
				Ok(db_json) => {
					let db_in = serde_json::from_str::<T>(db_json.as_str()).unwrap();

					info!("Read {}", &db_path);

					db_in
				}
				_ => failover(&db_path),
			}
		}
	}
}


/**
 * Instead of taking a &T, we take the locked atomic and handle errors accordingly right here.
 *
 * Only allow clearing poison if we're shutting down, otherwise leave it false.
 **/
pub fn save_db<T: Serialize>(db: &LockedAtomic<T>, clear_poison: bool) {
	if db.is_poisoned() {
		const IS_STABLE: bool = false;
		if IS_STABLE {
			error!(
				"DB was poisoned, can't clear it because we're in (stable) channel; so saving won't work.\n\
				This probaly happened because of an error.\n\
				Logging service will soon be implemented to notify of these."
			);
		} else if clear_poison {
			error!(
				"DB was poisoned, we'll clear it for now.\n\
				This probaly happened because of an error.\n\
				Logging service will soon be implemented to notify of these."
			);
			db.clear_poison();
		} else {
			error!("DB was poisoned, so we can't save.");
		}
	}


	#[cfg(debug_assertions)]
	let data = serde_json::to_string_pretty(&*db.read().unwrap()).unwrap();

	#[cfg(not(debug_assertions))]
	let data = serde_json::to_string(&*db.read().unwrap()).unwrap();
	

	let db_path = DB_PATH.clone();
	match fs::write(&db_path, &data) {
		Ok(()) => info!("DB saved on {db_path}"),
		Err(e) => {
			error!("Saving DB to path {db_path}: {e}");
		}
	};
}
