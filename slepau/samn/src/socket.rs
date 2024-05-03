use std::{collections::VecDeque, net::SocketAddr, sync::RwLock, time::Duration};

use axum::{
	extract::{
		ws::{Message, WebSocket, WebSocketUpgrade},
		ConnectInfo,
	},
	response::Response,
	Extension,
};

use common::{
	proquint::Proquint,
	socket::{MessageType, ResourceMessage, ResourceSender, SocketMessage},
	utils::LockedAtomic,
};
use futures::{sink::SinkExt, stream::StreamExt};
use log::{error, info};
use samn_common::node::{LimbId, NodeId};
use tokio::{sync::watch, time};

use auth::UserClaims;

use crate::{
	db::{NodeUiData, DB},
	views::{self, limb_history, LimbQuery},
};

pub async fn websocket_handler(
	ws: WebSocketUpgrade,
	Extension(_user): Extension<UserClaims>,
	Extension(db): Extension<LockedAtomic<DB>>,
	Extension(tx_resource): Extension<ResourceSender>,
	Extension(rx_shutdown): Extension<watch::Receiver<()>>,
	ConnectInfo(address): ConnectInfo<SocketAddr>,
) -> Response {
	info!("Opening Websocket with {} on {}.", &_user.user, address);
	ws.on_upgrade(move |socket| handle_socket(db, socket, _user, tx_resource, rx_shutdown, address))
}

async fn handle_socket(
	db: LockedAtomic<DB>,
	socket: WebSocket,
	user_claims: UserClaims,
	tx_resource: ResourceSender,
	mut rx_shutdown: watch::Receiver<()>,
	address: SocketAddr,
) {
	let user = &user_claims.user;
	// Create a new receiver for our Broadcast
	let mut rx_resource = tx_resource.subscribe();

	let (mut tx_socket, mut rx_socket) = socket.split();

	// Keep last resource id so when we're sending
	// a message in resource stream, we don't process
	// the message on the instance that sent it
	// (if it was incremented by that instance beforehand)
	let resource_id_last = RwLock::new(0);
	// Keep a list of explicitely acccessed chunks
	// So we don't give away all our public chunks to everyone
	// let access_list = Mutex::new(HashSet::default());

	let handle_incoming = |m| {
		if let Message::Text(m) = m {
			let m = serde_json::from_str::<SocketMessage>(&m);
			if m.is_err() {
				return None;
			}
			let m = m.unwrap();
			// let page_query = m.value.as_ref().and_then(|v| serde_json::from_str::<PageQuery>(v.as_str()).ok()).unwrap_or_default();
			let reply = |mut v: SocketMessage| {
				v.resource = m.resource.to_owned();
				v.id = m.id;
				// Send ok if id exists but message doesn't have any, and remove status if id doesn't exist
				match v.id {
					Some(_) => {
						if v._type.is_none() {
							v._type = Some(MessageType::Ok)
						}
					}
					None => {
						if v._type == Some(MessageType::Ok) {
							v._type = None;
						};
					}
				}
				Some(Message::Text(serde_json::to_string(&v).unwrap()))
			};
			let mut res = m.resource.split('/').collect::<VecDeque<_>>();
			let piece = res.pop_front();

			if piece == Some("commands") {
				return reply((&db.read().unwrap().commands()).into());
			} else if piece == Some("edit") {
				if let Some(node_id) = res
					.pop_front()
					.and_then(|piece| Proquint::<NodeId>::from_quint(piece).ok())
				{
					if let Some(ui_data) = m
						.value
						.to_owned()
						.and_then(|v| serde_json::from_str::<NodeUiData>(&v).ok())
					{
						db.write().unwrap().set_ui_data(node_id.inner(), ui_data)
					}
				}
			} else if piece == Some("views") {
				let piece = res.pop_front();
				if let Some(piece) = piece {
					if piece == "nodes" {
						if let Some(node_id) = res
							.pop_front()
							.and_then(|piece| Proquint::<NodeId>::from_quint(piece).ok())
						{
							if let Some(limb_id) = res.pop_front().and_then(|v| v.parse::<LimbId>().ok()) {
								let mut query = LimbQuery {
									node_id,
									limb_id,
									..Default::default()
								};
								if let Some(period) = res.pop_front().and_then(|v| v.parse().ok()) {
									query.period = period;
								}
								if let Some(limit) = res.pop_front().and_then(|v| v.parse().ok()) {
									query.limit = limit;
								}
								// Limb history
								// views/nodes/<node_id>/<limb_id>/period?/limit?
								return reply((&limb_history(query)).into());
							} else {
								// Node Detail
								// views/nodes/<node_id>
								return reply(
									(&views::node_previews(&db.read().unwrap(), format!("{}%", node_id)).get(&node_id)).into(),
								);
							}
						} else {
							// All nodes preview
							// views/nodes
							return reply((&views::node_previews(&db.read().unwrap(), "%".into())).into());
						}
					}
				}
			}

			error!("{m:?} unknown");
		}

		None
	};

	let handle_resource = |message: ResourceMessage| -> Result<Vec<String>, ()> {
		let mut messages = vec![];
		if let Some(users) = message.close_for_users {
			if users.contains(user) {
				return Err(());
			}
		}
		{
			// Only continue if the message's id is greater than our last processed id
			let mut resource_id_last = resource_id_last.write().unwrap();
			if message.id <= *resource_id_last {
				return Ok(messages);
			}
			*resource_id_last = message.id;
		}
		// Only continue if the connected user is part of the list of users in the message
		if let Some(users) = message.users {
			if !users.contains(user) {
				return Ok(messages);
			}
		}

		// info!("Triggered '{}' to '{}'", &message.message.resource, user);

		messages.push(serde_json::to_string(&message.message).unwrap());

		Ok(messages)
	};
	loop {
		tokio::select! {
			// Handles Websocket incomming
			m = rx_socket.next() => {
				if let Some(m) = m{

					if let Ok(m) = m {
						// info!("Received {m:?}");
						if let Some(m) = handle_incoming(m){
							tx_socket.send(m).await.unwrap();
						};
					}else{
						info!("Received Err from {address}, client disconnected");
						break;
					}
				}else{
					info!("Received None from {address}, client disconnected");
					break;
				}
			}
			// Handles resource incoming
			m = rx_resource.recv() => {
				if let Ok(m) = m {
					if let Ok(ms) = handle_resource(m){
						for m in ms {
							tx_socket.feed(Message::Text(m)).await.unwrap();
						}
						if let Err(err) = tx_socket.flush().await {
								info!("Got {err:?} while sending to {address}, assuming client disconnected");
								break;
						};
					}else{break;}

				}else{
					error!("Received Err resource {m:?} on {address}, closing connection.");
					match tx_socket.close().await{
						Ok(()) => {info!("Socket {address} closed successfully!")}
						Err(err) => {error!("Got {err:?} on {address} while closing");}
					}
					break;
				}
			}
			_ = rx_shutdown.changed() => {
				break;
			}
			// Send a ping message
			_ = time::sleep(Duration::from_secs(20u64)) => {
				tx_socket.send(Message::Ping(vec![50u8])).await.unwrap();
				continue;
			}
		}
	}

	info!("Closed socket with {user} on {address}");
}
