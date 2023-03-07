use serde::{Deserialize, Serialize};

pub mod cache;
pub mod http;
pub mod init;
pub mod proquint;
pub mod utils;
pub mod socket;

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct Cache {
	pub last_backup: u64,
}
