use axum::{
	extract::{Extension, Path},
	response::IntoResponse,
	Json,
};
use log::error;
use serde::Serialize;

use crate::utils::LockedAtomic;

/** Used as a magic static value for data cloning */
pub static MAGIC_BEAN: &str = "alkjgblnvcxlk_BANDFLKj";
/**
 * Endpoint allows other servers to clone this one's data
 */
pub async fn mirror_bean<T: Serialize>(
	Path(bean): Path<String>,
	Extension(db): Extension<LockedAtomic<T>>,
) -> Result<impl IntoResponse, impl IntoResponse> {
	if bean == *MAGIC_BEAN {
		Ok(Json(serde_json::to_value(&*db.read().unwrap()).unwrap()))
	} else {
		error!("Someone tried to access /mirror without bean.");
		Err("Who the F are you?")
	}
}
