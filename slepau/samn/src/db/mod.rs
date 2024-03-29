use std::{collections::HashMap, time::Instant};



#[derive(Debug)]
pub struct DB {
    pub heartbeats: HashMap::<u16, (Instant, u32)>,
	pub command_id: u8
}
impl Default for DB {
    fn default() -> Self {
        Self {
            heartbeats: Default::default(),
            command_id: 0,
        }
    }
}