use axum::{extract::Path, response::IntoResponse, Extension, Json};
use common::{utils::DbError, vreji::record_json};
use tokio::sync::{mpsc, oneshot};

use crate::{
	logging::db,
	radio::{CommandMessage, RadioSyncType},
};

pub async fn log_get(Path(key): Path<String>) -> Result<impl IntoResponse, DbError> {
	let db = db();
	let reader = db.get(key.as_str()).into_iter();
	let records_json = reader.map(record_json).collect::<Vec<_>>();

	Ok(Json(records_json))
}

pub async fn command(
	Extension(radio_tx): Extension<mpsc::Sender<RadioSyncType>>,
	Json(command): Json<CommandMessage>,
) -> Result<impl IntoResponse, DbError> {
	radio_tx.send((command, None)).await.unwrap();
	Ok(())
}

pub async fn command_response(
	Extension(radio_tx): Extension<mpsc::Sender<RadioSyncType>>,
	Json(command): Json<CommandMessage>,
) -> Result<impl IntoResponse, DbError> {
	let (tx, rx) = oneshot::channel();
	radio_tx.send((command, Some(tx))).await.unwrap();
	Ok(Json (rx.await.unwrap()))
}
