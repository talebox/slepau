use std::{
	collections::{HashMap, LinkedList},
	time::Instant,
};

use bimap::BiMap;
use samn_common::{
	node::{Command, NodeAddress, NodeId, Response},
	radio::DEFAULT_PIPE,
};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::radio::CommandMessage;

const HQADDRESS: u16 = 0x9797u16;
pub const HQ_PIPES: [u8; 6] = [
	DEFAULT_PIPE,
	DEFAULT_PIPE + 1,
	DEFAULT_PIPE + 2,
	DEFAULT_PIPE + 3,
	DEFAULT_PIPE + 4,
	DEFAULT_PIPE + 5,
];

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct NodeUiData {
	pub name: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct DB {
	/// Id <> Address
	pub addresses: BiMap<NodeId, NodeAddress>,
	pub node_ui_data: HashMap<NodeId, NodeUiData>,
	
	/// Instant (used to measure now - last)
	/// Last Message (u64)
	/// Uptime (u32)
	/// Heartbeat delay time in secs (u16)
	#[serde(skip)]
	pub heartbeats: HashMap<u32, (Instant, u64, u32, u16)>,
	#[serde(skip)]
	pub command_id: u8,
	#[serde(skip)]
	pub command_messages: LinkedList<(CommandMessage, Option<oneshot::Sender<Response>>)>,
	#[serde(skip)]
	pub response_callbacks: LinkedList<(u8, oneshot::Sender<Response>)>,

}

impl DB {
	pub fn maybe_queue_update(&mut self, id_node_db: u32) -> bool {
		if self
			.heartbeats
			.get(&id_node_db)
			.map(|(last, _, _, interval)| (Instant::now() - *last).as_secs() > (*interval * 3).into())
			.unwrap_or(true)
			&& self
				.command_messages
				.iter()
				.filter(|(m, _)| m.for_id.inner() == id_node_db)
				.count() < 2
		{
			self.command_messages.push_back((
				CommandMessage {
					for_id: id_node_db.into(),
					command: Command::Info,
				},
				None,
			));
			self.command_messages.push_back((
				CommandMessage {
					for_id: id_node_db.into(),
					command: Command::Limbs,
				},
				None,
			));
			true
		} else {
			false
		}
	}
	pub fn get_next_command_message(&mut self, id_node_db: u32) -> Option<(u8, CommandMessage)> {
		if let Some(i) = self
			.command_messages
			.iter()
			.position(|(m, _)| m.for_id.inner() == id_node_db)
		{
			let (message, callback) = self.command_messages.remove(i);
			let command_id = self.next_command_id();
			// Add callback to another array with an id
			// so we know what to call later if we receive a response
			if let Some(callback) = callback {
				self.response_callbacks.push_back((command_id, callback));
			}
			Some((command_id, message))
		} else {
			None
		}
	}
	pub fn get_response_callback(&mut self, id_command: u8) -> Option<oneshot::Sender<Response>> {
		// Remove all callbacks that are closed
		self.response_callbacks.retain(|(_, callback)| !callback.is_closed());
		if let Some(i) = self
			.response_callbacks
			.iter()
			.position(|(id_command_, _)| *id_command_ == id_command)
		{
			let (_, callback) = self.response_callbacks.remove(i);
			Some(callback)
		} else {
			None
		}
	}

	pub fn next_command_id(&mut self) -> u8 {
		let c = self.command_id;
		self.command_id = self.command_id.wrapping_add(1);
		c
	}
	pub fn commands(&self) -> Vec<&CommandMessage> {
		self.command_messages.iter().map(|(m, _)| m).collect()
	}

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
	pub fn set_ui_data(&mut self, node_id: NodeId, ui_data: NodeUiData) {
		self.node_ui_data.insert(node_id, ui_data);
	}
}

mod test {

	#[test]
	fn addresses() {
		use crate::db::{DB, HQADDRESS};
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
