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
	socket::{MessageType, ResourceMessage, ResourceSender, SocketMessage},
	utils::LockedAtomic,
};
use futures::{sink::SinkExt, stream::StreamExt};
use log::{error, info};
use serde_json::json;
use tokio::{sync::watch, time};

use auth::UserClaims;

use crate::db::{
	view::{Cursor, CursorQuery, MediaVec, SortType},
	MediaId, DB,
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
	ws.on_upgrade(move |socket| handle_socket(socket, _user, db, tx_resource, rx_shutdown, address))
}

async fn handle_socket(
	socket: WebSocket,
	user_claims: UserClaims,
	db: LockedAtomic<DB>,
	tx_resource: ResourceSender,
	mut rx_shutdown: watch::Receiver<()>,
	address: SocketAddr,
) {
	let user = &user_claims.user;
	// Create a new receiver for our Broadcast
	let mut rx_resource = tx_resource.subscribe();

	let (mut tx_socket, mut rx_socket) = socket.split();

	let get_all = || {
		let mut chunks: MediaVec = db.read().unwrap().get_all(user).into();
		chunks.sort(SortType::Created);

		json!(Vec::<crate::db::Media>::from(chunks))
	};
	// Like get_all, but paged
	let get_paged = |query: CursorQuery| {
		let mut chunks: MediaVec = db.read().unwrap().get_all(user).into();
		chunks.sort(SortType::Created);
		let iter = chunks.0.iter();
		let limit = query.limit;
		let cursor = query.cursor;
		let data = match cursor {
			Some(cursor) => match cursor {
				Cursor::Before(id) => iter
					.rev()
					.skip_while(|v| v.read().unwrap().id != id)
					.take(limit as usize)
					.map(|f| f.read().unwrap().clone())
					.collect::<Vec<_>>(),
				Cursor::After(id) => iter
					.skip_while(|v| v.read().unwrap().id != id)
					.take(limit as usize)
					.map(|f| f.read().unwrap().clone())
					.collect::<Vec<_>>(),
			},
			None => iter
				.take(limit as usize)
				.map(|f| f.read().unwrap().clone())
				.collect::<Vec<_>>(),
		};
		json!({"query": query,"data":data})
	};

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
			let mut piece = res.pop_front();

			if piece == Some("tasks") {
				return reply((&db.read().unwrap().tasks_len()).into());
			} else if piece == Some("media") {
				let root_id = res.pop_front().map(|id| MediaId::from_quint(id).expect("a ChunkId."));
				if let Some(id) = root_id {
					return reply((&db.read().unwrap().get(id).map(crate::db::Media::from)).into());
				} else {
					error!("Media request needs an id");
					return None;
				}
			} else if piece == Some("views") {
				piece = res.pop_front();
				// let _root_id = res.pop_front().map(|id| MediaId::from_quint(id).expect("a ChunkId."));

				if piece == Some("all") {
					piece = res.pop_front();
					if piece == Some("paged") {
						let cursor_query = m.value.and_then( |value|
							serde_json::from_str::<CursorQuery>(&value).ok()).unwrap_or_default();
						return reply((&get_paged(cursor_query)).into());
					}

					return reply((&get_all()).into());
				}
				error!("View '{piece:?}' not recognized");
				return None;
			} else if piece == Some("paged") {
				
			} else if piece == Some("user") {
				let stats = db.read().unwrap().user_stats(&user_claims.user);

				return reply((&stats).into());
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
