use serde::{Deserialize, Serialize};

pub mod cache;
pub mod init;
pub mod utils;
pub mod http;
pub mod proquint;

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct Cache {
	pub last_backup: u64,
}
