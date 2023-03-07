use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// A SocketMessage Type, either an Error, or Ok
///
/// (explicit confirmation, or explicit error)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum MessageType {
	// Id + Ok + Value?
	Ok,
	// Id + Err + Value?
	#[serde(rename = "Err")]
	Error,
}

/// A transactional type for WebSocket messages
///
/// A simpler HTTP I guess...
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct SocketMessage {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub id: Option<usize>,
	// Is this OK, or Error
	#[serde(rename = "type", skip_serializing_if = "Option::is_none")]
	pub _type: Option<MessageType>,
	/// What resource are we sending/receiving
	#[serde(skip_serializing_if = "String::is_empty")]
	pub resource: String,
	/// What value are we sending/receiving
	#[serde(skip_serializing_if = "Option::is_none")]
	pub value: Option<String>,
}
/**
 * (Value)
 */
impl<T: Serialize> From<&T> for SocketMessage {
	fn from(value: &T) -> Self {
		Self {
			value: serde_json::to_string(value).ok(),
			..Default::default()
		}
	}
}

/**
 * (Resource, Value)
 */
impl<T: Serialize> From<(&str, &T)> for SocketMessage {
	fn from((resource, value): (&str, &T)) -> Self {
		Self {
			resource: resource.into(),
			value: serde_json::to_string(value).ok(),
			..Default::default()
		}
	}
}

/**
 * (Type, Value)
 */
impl<T: Serialize> From<(MessageType, &T)> for SocketMessage {
	fn from((_type, value): (MessageType, &T)) -> Self {
		Self {
			_type: Some(_type),
			value: serde_json::to_string(value).ok(),
			..Default::default()
		}
	}
}
/**
 * (Type)
 */
impl From<MessageType> for SocketMessage {
	fn from(_type: MessageType) -> Self {
		Self {
			_type: Some(_type),
			..Default::default()
		}
	}
}

pub type ResourceSender = broadcast::Sender<ResourceMessage>;

#[derive(Clone, Debug)]
pub struct ResourceMessage {
	pub id: usize,
	pub users: Option<HashSet<String>>,
	/// If this is Some, sockets that have contained users will close.
	pub close_for_users: Option<HashSet<String>>,

	pub message: SocketMessage,
}
impl Default for ResourceMessage {
	fn default() -> Self {
		Self {
			id: resource_id_next(),
			users: Default::default(),
			message: Default::default(),
			close_for_users: Default::default(),
		}
	}
}
/**
 * (Message)
 */
impl From<SocketMessage> for ResourceMessage {
	fn from(message: SocketMessage) -> Self {
		Self {
			message,
			..Default::default()
		}
	}
}
/**
 * (Message, Users)
 */
impl From<(SocketMessage, HashSet<String>)> for ResourceMessage {
	fn from((message, users): (SocketMessage, HashSet<String>)) -> Self {
		Self {
			message,
			users: Some(users),
			..Default::default()
		}
	}
}

/**
 * Resource
 */
impl From<&str> for ResourceMessage {
	fn from(value: &str) -> Self {
		Self {
			message: SocketMessage {
				resource: value.into(),
				..Default::default()
			},
			..Default::default()
		}
	}
}

/**
 * (Resource, Users)
 */
impl From<(&str, HashSet<String>)> for ResourceMessage {
	fn from((resource, users): (&str, HashSet<String>)) -> Self {
		Self {
			message: SocketMessage {
				resource: resource.into(),
				..Default::default()
			},
			users: Some(users),
			..Default::default()
		}
	}
}
/**
 * (Resource, Users, Value)
 */
impl<T: Serialize> From<(&str, HashSet<String>, &T)> for ResourceMessage {
	fn from((resource, users, value): (&str, HashSet<String>, &T)) -> Self {
		Self {
			message: SocketMessage::from((resource, value)),
			users: Some(users),
			..Default::default()
		}
	}
}

static mut RESOURCE_ID: usize = 0;
fn resource_id_next() -> usize {
	unsafe {
		let j = RESOURCE_ID;
		RESOURCE_ID += 1;
		j
	}
}
