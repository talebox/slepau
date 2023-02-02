use serde::{Deserialize, Serialize};

pub mod cache;
pub mod init;
pub mod utils;

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct Cache {
	pub last_backup: u64,
}
