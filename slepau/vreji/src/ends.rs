use std::{
	collections::{BTreeMap, HashMap, HashSet},
	time::SystemTime, net::Ipv4Addr,
};

use auth::UserClaims;
use axum::{
	extract::{Extension, Path},
	response::IntoResponse,
	Json, TypedHeader,
};
use serde_json::{json, Value};
use sonnerie::{record, DatabaseReader, Record};

use common::{
	socket::{ResourceMessage, ResourceSender},
	utils::{DbError, LockedAtomic},
};

// use common::vreji::DB_PATH_LOG;
use lazy_static::lazy_static;
use serde::Deserialize;

fn db() -> DatabaseReader {
	sonnerie::DatabaseReader::new(common::vreji::DB_PATH_LOG.as_path()).unwrap()
}

fn record_json(r: Record) -> Value {
	let mut v = vec![];
	v.push(json!(r.time().timestamp_nanos()));
	for (idx, c) in r.format().chars().enumerate() {
		v.push(match c {
			'f' => json!(r.get::<f32>(idx)),
			'F' =>  json!(r.get::<f64>(idx)),
			'i' =>  json!(r.get::<i32>(idx)),
			'I' =>  json!(r.get::<i64>(idx)),
			'u' =>  json!(r.get::<u32>(idx)),
			'U' =>  json!(r.get::<u64>(idx)),
			's' => json!(r.get::<&str>(idx).escape_default().to_string()),
			a => panic!("unknown format column '{a}'"),
		});
	}
	json!(v)
}

/// Gets logs
pub async fn log_get(Path(key): Path<String>) -> Result<impl IntoResponse, DbError> {
	let db = db();
	let reader = db.get(key.as_str()).into_iter();
	let records_json = reader.map(record_json).collect::<Vec<_>>();

	Ok(Json(records_json))
}

pub async fn ips() -> impl IntoResponse {
	let db = db();
	Json(
		db.get_range("auth"..="b")
			.into_iter()
			.fold(HashMap::new(), |mut acc, r| {
				let key = Ipv4Addr::from(r.get::<u32>(0)).to_string();
				let time = r.timestamp_nanos();
				acc
					.entry(key)
					.and_modify(|v: &mut Vec<u64>| {
						if v[0] < time {
							v[0] = time
						};
						v[1] += 1;
					})
					.or_insert(vec![time, 1]);
				acc
			}),
	)
}
