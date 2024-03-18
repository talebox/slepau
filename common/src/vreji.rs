use std::net::{IpAddr, Ipv4Addr};

use chrono::Utc;
use lazy_static::lazy_static;
use serde_json::json;
use sonnerie::{record, CreateTx, Record};

use crate::proquint::Proquint;

lazy_static! {
	pub static ref DB_PATH_LOG: std::path::PathBuf =
		std::path::PathBuf::from(std::env::var("DB_PATH_LOG").unwrap().as_str());
}

fn transaction() -> CreateTx {
	sonnerie::CreateTx::new(DB_PATH_LOG.as_path()).unwrap()
}
fn commit(t: CreateTx) {
	t.commit().unwrap();
}
pub fn ip_to_u32(ip: IpAddr) -> u32 {
	match ip {
		IpAddr::V4(v4) => v4.into(),
		IpAddr::V6(_) => 0,
	}
}

pub fn db() -> sonnerie::DatabaseReader {
	sonnerie::DatabaseReader::new(DB_PATH_LOG.as_path()).unwrap()
}

pub fn record_json(r: Record) -> serde_json::Value {
	let mut v = vec![];
	v.push(json!(r.time().timestamp_nanos()));
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

pub struct RecordValues {
	pub key: String,
	pub time: u64,
	pub ip: String,
	pub user: Option<String>,
	pub id: Option<String>,
}
impl From<&Record> for RecordValues {
	fn from(r: &Record) -> Self {
		let f = r.format();
		let user = f
			.chars()
			.nth(1)
			.and_then(|c| if c == 's' { Some(r.get::<String>(1)) } else { None });
		let id = f.chars().nth(2).and_then(|c| {
			if c == 'U' {
				let v = r.get::<u64>(2);
				Some(if v <= u32::MAX.into() {
					Proquint::<u32>::from(v as u32).to_quint()
				} else {
					Proquint::<u64>::from(v).to_quint()
				})
			} else {
				None
			}
		});
		Self {
			key: r.key().into(),
			time: r.timestamp_nanos(),
			ip: Ipv4Addr::from(r.get::<u32>(0)).to_string(),
			user,
			id,
		}
	}
}

pub fn log_ip(name: &str, ip: IpAddr) {
	let mut t = transaction();
	t.add_record(name, Utc::now().naive_utc(), record(ip_to_u32(ip)))
		.unwrap();
	commit(t);
}
pub fn log_ip_user(name: &str, ip: IpAddr, user: &str) {
	let mut t = transaction();
	t.add_record(name, Utc::now().naive_utc(), record(ip_to_u32(ip)).add(user))
		.unwrap();
	commit(t);
}
pub fn log_ip_user_id(name: &str, ip: IpAddr, user: &str, id: u64) {
	let mut t = transaction();
	t.add_record(name, Utc::now().naive_utc(), record(ip_to_u32(ip)).add(user).add(id))
		.unwrap();
	commit(t);
}
pub fn log_ip_user_id_bytes(name: &str, ip: IpAddr, user: &str, id: u64, size: u32) {
	let mut t = transaction();
	t.add_record(
		name,
		Utc::now().naive_utc(),
		record(ip_to_u32(ip)).add(user).add(id).add(size),
	)
	.unwrap();
	commit(t);
}
