use std::{collections::HashMap, time::Instant};

use bimap::BiMap;
use common::proquint::Proquint;
use samn_common::{
	node::{NodeAddress, NodeId},
	radio::DEFAULT_PIPE,
};
use serde::{Deserialize, Serialize};

const HQADDRESS: u16 = 0x9797u16;
pub const HQ_PIPES: [u8; 6] = [
	DEFAULT_PIPE,
	DEFAULT_PIPE + 1,
	DEFAULT_PIPE + 2,
	DEFAULT_PIPE + 3,
	DEFAULT_PIPE + 4,
	DEFAULT_PIPE + 5,
];

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct DB {
	/// Id <> Address
	pub addresses: BiMap<NodeId, NodeAddress>,
	#[serde(skip)]
	/// Instant (used to measure now - last)
	/// Last Message (u64)
	/// Uptime (u32)
	/// Heartbeat delay time in secs (u16)
	pub heartbeats: HashMap<u32, (Instant, u64, u32, u16)>,
	#[serde(skip)]
	pub command_id: u8,
}

impl DB {
	// We'll give the nrf24 all addresses > HQADDRESS
	pub fn nrf24_addresses(&self) -> Vec<u16> {
		self
			.addresses
			.iter()
			.filter(|(_, addr)| **addr > HQADDRESS)
			.map(|(_, addr)| addr.clone())
			.collect()
	}

	// We'll give the cc1101 all addresses < HQADDRESS
	pub fn cc1101_addresses(&self) -> Vec<u16> {
		self
			.addresses
			.iter()
			.filter(|(_, addr)| **addr < HQADDRESS)
			.map(|(_, addr)| addr.clone())
			.collect()
	}

	/// Issuing an address won't be easy
	///
	/// We need to pull the corresponding address of each
	pub fn issue_address(&mut self, id: NodeId, is_nrf: bool) -> NodeAddress {
		if let Some(address) = self.addresses.get_by_left(&id).cloned() {
			address
		} else {
			let mut addresses = if is_nrf {
				self.nrf24_addresses()
			} else {
				self.cc1101_addresses()
			};

			addresses.sort();
			let new_address = if is_nrf {
				addresses.last().cloned().unwrap_or(HQADDRESS) + 1
			} else {
				addresses.first().cloned().unwrap_or(HQADDRESS) - 1
			};

			self.addresses.insert(id, new_address);

			new_address
		}
	}
}

mod test {
	use super::DB;
	use crate::db::HQADDRESS;

	#[test]
	fn addresses() {
		let mut db = DB::default();

		// New address
		assert_eq!(db.issue_address(16, true), (HQADDRESS + 1));
		// Address 1 above the one before
		assert_eq!(db.issue_address(22, true), (HQADDRESS + 2));
		// Same address as before, nodeid exists
		assert_eq!(db.issue_address(22, false), (HQADDRESS + 2));
		// For cc1101 is 1 below
		assert_eq!(db.issue_address(324, false), (HQADDRESS - 1));
	}
}
