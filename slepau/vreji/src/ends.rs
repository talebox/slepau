use std::{
	collections::{BTreeMap, HashMap, HashSet},
	net::Ipv4Addr,
	time::SystemTime,
};

use auth::UserClaims;
use axum::{
	extract::{Extension, Path, Query},
	response::IntoResponse,
	Json, TypedHeader,
};
use serde_json::{json, Value};
use sonnerie::{record, DatabaseReader, Record, Wildcard};

use common::{
	socket::{ResourceMessage, ResourceSender},
	utils::{DbError, LockedAtomic},
	vreji::{db, record_json, RecordValues},
};

// use common::vreji::DB_PATH_LOG;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

/// Gets logs
pub async fn log_get(Path(key): Path<String>) -> Result<impl IntoResponse, DbError> {
	let db = db();
	let reader = db.get(key.as_str()).into_iter();
	let records_json = reader.map(record_json).collect::<Vec<_>>();

	Ok(Json(records_json))
}

pub async fn by_ip() -> impl IntoResponse {
	let db = db();
	Json(
		db.get_filter(&Wildcard::new("%"))
			.into_iter()
			.fold(HashMap::new(), |mut acc, r| {
				let r = RecordValues::from(&r);
				let ip_entry = acc
					.entry(r.ip)
					.or_insert((r.time, 0 as u64, HashMap::new(), HashMap::new()));
				if ip_entry.0 < r.time {
					ip_entry.0 = r.time
				};
				ip_entry.1 += 1;

				let action_entry = ip_entry.2.entry(r.key).or_insert(0);
				*action_entry += 1;

				if let Some(user) = r.user {
					let user_entry = ip_entry.3.entry(user).or_insert(0);
					*user_entry += 1;
				}

				acc
			}),
	)
}
pub async fn by_user() -> impl IntoResponse {
	let db = db();
	Json(
		db.get_filter(&Wildcard::new("%"))
			.into_iter()
			.fold(HashMap::new(), |mut acc, r| {
				let r = RecordValues::from(&r);
				if let Some(user) = r.user {
					let user_entry = acc
						.entry(user)
						.or_insert((r.time, 0 as u64, HashMap::new(), HashMap::new()));
					if user_entry.0 < r.time {
						user_entry.0 = r.time
					};
					user_entry.1 += 1;
					let key_entry = user_entry.2.entry(r.key).or_insert(0);
					let ip_entry = user_entry.3.entry(r.ip).or_insert(0);
					*key_entry += 1;
					*ip_entry += 1;
				}
				acc
			}),
	)
}

#[derive(Deserialize)]
#[serde(default)]
pub struct RecordFilter {
	ip: Option<String>,
	user: Option<String>,
	id: Option<String>,
}
impl RecordFilter {
	fn matches(&self, r: &RecordValues) -> bool {
		self.ip.as_deref().map(|ip| ip == &r.ip).unwrap_or(true)
			&& self
				.user
				.as_deref()
				.map(|user| Some(user) == r.user.as_deref())
				.unwrap_or(true)
			&& self.id.as_deref().map(|id| Some(id) == r.id.as_deref()).unwrap_or(true)
	}
}
impl From<&StatQuery> for RecordFilter {
	fn from(q: &StatQuery) -> Self {
		Self {
			ip: q.ip.to_owned(),
			user: q.user.to_owned(),
			id: q.id.to_owned(),
		}
	}
}
impl Default for RecordFilter {
	fn default() -> Self {
		Self {
			ip: None,
			user: None,
			id: None,
		}
	}
}

pub async fn by_anything(Query(query): Query<RecordFilter>) -> impl IntoResponse {
	Json(query.user)
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct StatQuery {
	ip: Option<String>,
	user: Option<String>,
	id: Option<String>,

	key: String,
	period: usize, // in sec
	limit: usize,  // in # of periods
	total: bool,   // total instead of grouped by actions
}
impl Default for StatQuery {
	fn default() -> Self {
		Self {
			ip: None,
			user: None,
			id: None,

			key: "%".into(),
			period: 3600, // an hour
			limit: 24,    // 24 periods (hours, 1 day)
			total: false,
		}
	}
}

/// Get chunk's stats page
pub async fn stats(Query(query): Query<StatQuery>) -> impl IntoResponse {
	let db = db();
	let now = SystemTime::now()
		.duration_since(SystemTime::UNIX_EPOCH)
		.unwrap()
		.as_secs();

	let wildcard = Wildcard::new(&query.key);

	Json(db.get_filter(&wildcard).into_iter().fold(HashMap::new(), |mut acc, r| {
		let r = RecordValues::from(&r);
		if !RecordFilter::from(&query).matches(&r) {
			return acc;
		}

		let key = if query.total { "Total".into() } else { r.key };
		let time = r.time / 1_000_000_000; // Seconds
		let time_diff = now - time;
		let values = acc.entry(key).or_insert(vec![0; query.limit]);
		if time_diff as usize >= query.limit * query.period {
			return acc;
		}
		values[time_diff as usize / query.period] += 1;
		return acc;
	}))
}
