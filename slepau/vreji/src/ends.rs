use std::{
	collections::{BTreeMap, HashMap, HashSet},
	time::SystemTime,
};

use auth::UserClaims;
use axum::{
	extract::{Extension, Path},
	response::IntoResponse,
	Json, TypedHeader,
};
use sonnerie::{record, DatabaseReader};

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

/// Gets logs
pub async fn log_get(Path(key): Path<String>) -> Result<impl IntoResponse, DbError> {
	let db = db();
	let reader = db.get(key.as_str()).into_iter();
	let records_str = reader.map(|v| format!("{v:?}")).collect::<Vec<_>>().join("\n");
	Ok(records_str)
}

pub async fn ips() -> impl IntoResponse {
	let db = db();
	Json(
		db.get_range("auth"..="b")
			.into_iter()
			.fold(HashMap::new(), |mut acc, r| {
				let key = if r.key() == "auth_logout" {
					r.get::<String>(0)
				} else {
					r.get::<String>(1)
				};
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

// /// Pushes a new log entry
// pub async fn log_post(
//     Path(key): Path<String>,
//     Path(format): Path<String>,
//     Path(data): Path<String>,
// ) -> Result<impl IntoResponse, DbError> {
//     let mut transaction = sonnerie::CreateTx::new(DB_PATH.as_path()).unwrap();
//     // transaction.add_record(key.as_str(), chrono::Utc::now().naive_utc(), record("Hello World!")).unwrap();
//     transaction.add_record_raw(key.as_str(), format.as_str(), data.as_ref()).unwrap();
//     transaction.commit().unwrap();
//     Ok(())
// }
