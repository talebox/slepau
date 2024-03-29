use lazy_static::lazy_static;
use samn_common::node::{Limb, NodeInfo};
use serde_json::json;
use sonnerie::{CreateTx, Record};


lazy_static! {
	pub static ref DB_PATH_LOG: std::path::PathBuf =
		std::path::PathBuf::from(std::env::var("DB_PATH_LOG").unwrap().as_str());
}

/// Creates a new sonnerie DB at DB_PATH_LOG if non-existent.
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