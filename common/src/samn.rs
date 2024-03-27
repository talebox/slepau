//! The definition for samn's database values
//! How are we to store state?
//! We should make an entry for every sensor report on each device
//! So... the key should be <deviceId(u16)>
//! And the values would be defined by the type of sensor and/or what values they have
//! 

pub struct RecordValues {
	pub key: String,
	pub time: u64,
	pub sensor_type: usize,
	pub user: Option<String>,
	pub id: Option<String>,
}