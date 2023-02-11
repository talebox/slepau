use crate::{
	utils::{get_secs, LockedAtomic, DB_BACKUP_FOLDER, SECS_IN_DAY, SECS_IN_HOUR, SECS_START_OF_TALEBOX},
	Cache,
};
use log::{error, info};
use serde::Serialize;
use std::{fs, path::Path, time::Duration};
use tokio::{sync::watch, time};

pub async fn backup_service<T: Serialize>(
	cache: LockedAtomic<Cache>,
	db: LockedAtomic<T>,
	mut shutdown_rx: watch::Receiver<()>,
) {
	let backup_folder = Path::new(DB_BACKUP_FOLDER.as_str());
	if !backup_folder.is_dir() {
		fs::create_dir(backup_folder).unwrap();
		info!("Created {backup_folder:?}.");
	}

	loop {
		let wait =
		// Last backup
			cache.read().unwrap().last_backup as i128
			// Minus seconds now
			- get_secs() as i128
			// Plus 2 hours
			+ (SECS_IN_HOUR as i128 * 2);

		if wait <= 0 {
			let secs = get_secs();
			cache.write().unwrap().last_backup = get_secs();

			let backup_file = backup_folder.join(format!(
				"{}.json",
				(secs - SECS_START_OF_TALEBOX) / SECS_IN_DAY /*Closest number to days since EPOCH to lower that to something more readable */
			));

			let dbdata = serde_json::to_string(&*db.read().unwrap()).unwrap();

			if let Err(err) = fs::write(&backup_file, dbdata) {
				error!("Couldn't backup to: {err:?}");
			} else {
				info!("Backed up to {backup_file:?}.");
			}
		} else {
			info!("Waiting {}h till next backup", wait / SECS_IN_HOUR as i128);
			tokio::select! {
				_ = time::sleep(Duration::from_secs(wait as u64)) => {
					continue;
				}
				_ = shutdown_rx.changed() => {
					break;
				}
			}
		}
	}
}
