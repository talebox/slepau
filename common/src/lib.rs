use serde::{Deserialize, Serialize};

pub mod cache;
pub mod http;
pub mod init;
pub mod proquint;
pub mod socket;
pub mod utils;
pub mod vreji;
pub mod samn;
pub mod sonnerie;

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct Cache {
	pub last_backup: u64,
}
