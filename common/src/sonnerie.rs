use std::time::Duration;

use lazy_static::lazy_static;
use log::info;
use serde_json::json;
use sonnerie::{CreateTx, Record};

use crate::utils::SECS_IN_HOUR;


lazy_static! {
	pub static ref DB_PATH_LOG: std::path::PathBuf =
		std::path::PathBuf::from(std::env::var("DB_PATH_LOG").unwrap().as_str());
}

/// Creates a new sonnerie DB at DB_PATH_LOG if non-existent.
/// 
/// Make sure DB_PATH_LOG resolves to an existing directory tho.
pub fn init() {
	let mut main = DB_PATH_LOG.clone();
	main.push("main");
	if !main.exists() {
		match std::fs::OpenOptions::new().create(true).write(true).open(main.clone()) {
			Ok(_) => {println!("Created new db at {:?}", main);}
			Err(err) => {println!("Coudn't create db at {:?} because {:?}", main, err);}
		}
	}
}

pub fn transaction() -> CreateTx {
	sonnerie::CreateTx::new(DB_PATH_LOG.as_path()).unwrap()
}
pub fn commit(t: CreateTx) {
	t.commit().unwrap();
}
pub fn db() -> sonnerie::DatabaseReader {
	sonnerie::DatabaseReader::new(DB_PATH_LOG.as_path()).unwrap()
}

pub fn record_json(r: Record) -> serde_json::Value {
	let mut v = vec![];
	v.push(json!(r.time().and_utc().timestamp_nanos_opt()));
	for (idx, c) in r.format().chars().enumerate() {
		v.push(match c {
			'f' => json!(r.get::<f32>(idx)),
			'F' => json!(r.get::<f64>(idx)),
			'i' => json!(r.get::<i32>(idx)),
			'I' => json!(r.get::<i64>(idx)),
			'u' => json!(r.get::<u32>(idx)),
			'U' => json!(r.get::<u64>(idx)),
			's' => json!(r.get::<&str>(idx).escape_default().to_string()),
			a => panic!("unknown format column '{a}'"),
		});
	}
	json!(v)
}

pub async fn compact_service(
	mut shutdown_rx: tokio::sync::watch::Receiver<()>,
) {
	
	loop {
		match sonnerie::compact(DB_PATH_LOG.as_path(), true) {
			Ok(v) => info!("Compacted {} records.", v),
			Err(err) => log::error!("Couldn't compact {err:?}")
		}
		let wait = Duration::from_secs( SECS_IN_HOUR * 2);
		info!("Next compaction in {}h", wait.as_secs() / SECS_IN_HOUR);
		tokio::select! {
			_ = tokio::time::sleep(wait) => {
			}
			_ = shutdown_rx.changed() => {
				break;
			}
		}
	}
}