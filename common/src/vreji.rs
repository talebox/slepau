use std::net::{IpAddr, Ipv4Addr};

use chrono::Utc;
use sonnerie::{record, Record};

use crate::{proquint::Proquint, sonnerie::{commit, transaction}};

pub fn ip_to_u32(ip: IpAddr) -> u32 {
	match ip {
		IpAddr::V4(v4) => v4.into(),
		IpAddr::V6(_) => 0,
	}
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
