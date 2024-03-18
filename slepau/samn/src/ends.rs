use axum::{extract::Path, response::IntoResponse, Json};
use common::{utils::DbError, vreji::record_json};

use crate::logging::{db};

pub async fn log_get(Path(key): Path<String>) -> Result<impl IntoResponse, DbError> {
	let db = db();
	let reader = db.get(key.as_str()).into_iter();
	let records_json = reader.map(record_json).collect::<Vec<_>>();

	Ok(Json(records_json))
}