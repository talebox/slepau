use std::fs;

use log::{error, info};

use crate::{utils::CACHE_PATH, Cache};

impl Cache {
	pub fn init() -> Self {
		fs::read(CACHE_PATH.clone())
			.map(|v| serde_json::from_slice::<Cache>(v.as_ref()).unwrap())
			.unwrap_or_default()
	}
	pub fn save(&self) {
		let s = serde_json::to_string_pretty(self).unwrap();
		if let Err(err) = fs::write(CACHE_PATH.clone(), s) {
			error!("Couldn't write cache: {err:?}");
		} else {
			info!("Saved cache -> {}", CACHE_PATH.as_str());
		}
	}
}
