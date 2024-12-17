use std::{
	collections::{HashMap, LinkedList},
	time::Instant,
};

use bimap::BiMap;
use samn_common::{
	node::{Board, Command, Limb, NodeAddress, NodeId, NodeInfo, Response},
	radio::DEFAULT_PIPE,
};
use schedule::Schedule;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::radio::{CommandMessage, RadioSyncType};

const HQADDRESS: u16 = 0x9797u16;
pub const HQ_PIPES: [u8; 2] = [
	DEFAULT_PIPE,
	DEFAULT_PIPE,
	// DEFAULT_PIPE + 1,
	// DEFAULT_PIPE + 2,
	// DEFAULT_PIPE + 3,
	// DEFAULT_PIPE + 4,
];

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct NodeUiData {
	pub name: String,
}

pub mod schedule;


#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct DB {
	/// Id <> Address
	pub addresses: BiMap<NodeId, NodeAddress>,
	pub node_ui_data: HashMap<NodeId, NodeUiData>,
	pub schedule_raw: String,

	/// Instant (used to measure now - last), a fast way of checking elapsed time
	/// Last Message (u64) unix system time in nanoseconds
	///
	/// Uptime (u32) uptime of the node in seconds
	/// NodeInfo, heartbeat_interval & board version
	#[serde(skip)]
	pub radio_info: HashMap<NodeId, (Instant, u64, u32, Option<NodeInfo>)>,
	/// This is a cache for limbs, node_previews is the only allowed to create a new record.
	/// A zero length means for node_previews that it should recurse the logs and create the records.
	/// 
	/// Upon a radio message if a record exists it is updated with limbs.
	/// 
	/// This allows node_preview to not have to recurse the logs every time, making pulling the latest data
	/// incredibly fast.
	#[serde(skip)]
	pub limbs_cache: HashMap<NodeId, Vec<Limb>>,

	#[serde(skip)]
	pub command_id: u8,

	// Normal command messages get sent out after a message from a node
	#[serde(skip)]
	pub command_messages: LinkedList<(CommandMessage, Option<oneshot::Sender<Response>>)>,

	/// Instant command messages get sent out immediately
	#[serde(skip)]
	pub command_messages_instant: LinkedList<(CommandMessage, Option<oneshot::Sender<Response>>)>,

	#[serde(skip)]
	pub response_callbacks: LinkedList<(u8, oneshot::Sender<Response>)>,

	pub schedule: Schedule
}

impl DB {
	pub fn maybe_queue_update(&mut self, id_node_db: u32) -> bool {
		// Queue an update if node hasn't reported for > 3 * heartbeat_interval.
		// and there's 0 messages queued.
		if self
			.radio_info
			.get(&id_node_db)
			.map(|(last, _, _, info)| {
				if let Some(info) = info {
					(Instant::now() - *last).as_secs() > (info.heartbeat_interval * 3).into()
				} else {
					true
				}
			})
			.unwrap_or(true)
			&& self
				.command_messages
				.iter()
				.filter(|(m, _)| m.for_id.inner() == id_node_db)
				.count() == 0
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
	pub fn get_next_instant_command_message(&mut self) -> Option<(u8, NodeAddress, CommandMessage)> {
		// First we figure out command, then get its node address
		self
			.command_messages_instant
			.pop_front()
			.and_then(|(message, callback)| {
				self
					.addresses
					.get_by_left(&message.for_id.inner())
					.map(|node_address| (message, callback, *node_address))
			})
			.map(|(message, callback, node_address)| {
				// Then we process the command and send all we need back
				let command_id = self.next_command_id();
				if let Some(callback) = callback {
					self.response_callbacks.push_back((command_id, callback));
				}
				(command_id, node_address, message)
			})
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
		self.command_id = (self.command_id + 1) % samn_common::node::COMMAND_ID_MAX;
		c
	}
	pub fn commands(&self) -> Vec<&CommandMessage> {
		self.command_messages.iter().map(|(m, _)| m).collect()
	}

	/// Picks where to place this command:
	/// - either as an instant command, gets sent out immediately
	/// - or as a command that gets sent out after a message from a node
	///
	/// Returns wether a non-instant command was queued.
	pub fn queue_command(&mut self, command: RadioSyncType) -> bool {
		if let Some((_, _, _, Some(info))) = self.radio_info.get(&command.0.for_id.inner()) {
			if matches!(info.board, Board::SamnDC | Board::SamnSwitch) {
				self.command_messages_instant.push_back(command);
				return false;
			}
		}
		self.command_messages.push_back(command);
		true
	}

	// We'll give the nrf24 all addresses > HQADDRESS
	pub fn nrf24_addresses(&self) -> Vec<u16> {
		self
			.addresses
			.iter()
			.filter(|(_, addr)| **addr > HQADDRESS)
			.map(|(_, addr)| *addr)
			.collect()
	}

	// We'll give the cc1101 all addresses < HQADDRESS
	pub fn cc1101_addresses(&self) -> Vec<u16> {
		self
			.addresses
			.iter()
			.filter(|(_, addr)| **addr < HQADDRESS)
			.map(|(_, addr)| *addr)
			.collect()
	}

	/// Issuing an address won't be easy
	///
	/// We need to pull the corresponding address of each nrf/cc1101
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

#[cfg(test)]
mod tests;
