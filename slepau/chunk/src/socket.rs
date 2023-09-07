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
	utils::LockedAtomic, vreji::log_ip_user_id,
};
use futures::{sink::SinkExt, stream::StreamExt};
use log::{error, info};
use serde_json::{json, Value};
use tokio::{sync::watch, time};

use axum_client_ip::InsecureClientIp;
type ClientIp = InsecureClientIp;

use auth::UserClaims;

use crate::db::{
	chunk::ChunkId,
	dbchunk::DBChunk,
	view::{ChunkValue, ChunkVec, ChunkView, SortType, ViewType},
	DB,
};

pub async fn websocket_handler(
	ws: WebSocketUpgrade,
	Extension(_user): Extension<UserClaims>,
	Extension(db): Extension<LockedAtomic<DB>>,
	Extension(tx_r): Extension<ResourceSender>,
	Extension(shutdown_rx): Extension<watch::Receiver<()>>,
	ip: ClientIp,
	ConnectInfo(address): ConnectInfo<SocketAddr>,
) -> Response {
	info!("Opening Websocket with {} on {}.", &_user.user, address);
	ws.on_upgrade(move |socket| handle_socket(ip, socket, _user, db, tx_r, shutdown_rx, address))
}

async fn handle_socket(
	ip: ClientIp,
	socket: WebSocket,
	user_claims: UserClaims,
	db: LockedAtomic<DB>,
	tx_resource: ResourceSender,
	mut shutdown_rx: watch::Receiver<()>,
	address: SocketAddr,
) {
	let user = &user_claims.user;
	// Create a new receiver for our Broadcast
	let mut rx_resource = tx_resource.subscribe();

	let (mut tx_socket, mut rx_socket) = socket.split();

	let get_notes = || {
		let mut chunks: ChunkVec = db.write().unwrap().get_chunks(user).into();
		chunks.sort(SortType::Modified);
		let chunks = chunks.0;
		// maybe_paginate((query, chunks, &|v| ChunkView::from((v, user.as_str(), ViewType::Notes))))
		let chunks = chunks
			.into_iter()
			.map(|v| ChunkView::from((v, user.as_str(), ViewType::Notes)))
			.collect::<Vec<_>>();
		json!(chunks)
	};

	// [[parent,parent], [child,child]]
	let get_subtree = |root: Option<ChunkId>, view_type: ViewType| {
		let root = root.and_then(|id| db.try_read().unwrap().get_chunk(id, user));
		let subtree = db.try_read().unwrap().subtree(
			root.as_ref(),
			&user.as_str().into(),
			&|v| {
				let mut vec = ChunkVec::from(v);
				vec.sort(SortType::ModifiedDynamic(user.as_str().into()));
				vec.into()
			},
			&|v| json!(ChunkView::from((v, user.as_str(), view_type))),
			1,
		);
		json!(subtree)
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

			if piece == Some("chunks") {
				if let Some(id) = res.pop_front().map(|id| ChunkId::from_quint(id).expect("a ChunkId.")) {
					piece = res.pop_front();

					if piece == Some("value") {
						if let Some(value) = m.value {
							// User wants to change a value
							let db_chunk: DBChunk = (id, value.as_str()).into();
							match db.write().unwrap().update_chunk(db_chunk, user) {
								Ok((users_to_notify, diff, db_chunk)) => {
									let users = db_chunk.read().unwrap().access_users();
									let m = ResourceMessage::from((format!("chunks/{}/value/diff", id).as_str(), users.clone(), &diff));
									{
										// Update our resource_id_last so we don't send the same data back when sending a signal to tx_resource
										let mut resource_id_last = resource_id_last.write().unwrap();
										*resource_id_last = m.id;
									}
									tx_resource.send(m).unwrap();
									tx_resource
										.send(ResourceMessage::from((
											format!("chunks/{}", id).as_str(),
											users,
											&ChunkView::from((db_chunk, user.as_str(), ViewType::Edit)),
										)))
										.unwrap();

									if !users_to_notify.is_empty() {
										tx_resource
											.send(ResourceMessage::from(("chunks", users_to_notify)))
											.unwrap();
									}

									log_ip_user_id("chunk_edit", ip.0, &user_claims.user, id.inner().into());
									return reply(MessageType::Ok.into());
								}
								Err(err) => {
									log_ip_user_id("chunk_edit_error", ip.0, &user_claims.user, id.inner().into());
									return reply((MessageType::Error, &format!("{err:?}")).into())
								},
							}
						} else {
							// Request for "chunks/<id>/value"
							if let Some(v) = db.read().unwrap().get_chunk(id, user) {
								return reply((&ChunkValue::from(v)).into());
							}
						}
					} else if piece.is_none() {
						if let Some(v) = db.read().unwrap().get_chunk(id, user) {
							return reply((&ChunkView::from((v, user.as_str(), ViewType::Edit))).into());
						}
					}

					return reply((MessageType::Error, &"NotFound".to_string()).into());
				} else {
					// Request for "chunks"
					// return reply((&get_notes()).into());
				}
			} else if piece == Some("views") {
				piece = res.pop_front();
				let root_id = res.pop_front().map(|id| ChunkId::from_quint(id).expect("a ChunkId."));
				if piece == Some("notes") {
					return reply((&get_notes()).into());
				} else if piece == Some("well") {
					return reply((&get_subtree(root_id, ViewType::Well)).into());
				} else if piece == Some("graph") {
					return reply((&get_subtree(root_id, ViewType::Graph)).into());
				}
				error!("View needs name");
				return None;
			} else if piece == Some("user") {
				let mut user = json!(&user_claims);
				if let Value::Object(mut user_o) = user {
					let mut db = db.write().unwrap();
					let chunks = db.get_chunks(&user_claims.user);
					user_o.insert("notes_visible".into(), chunks.len().into());
					user_o.insert(
						"notes_owned".into(),
						chunks
							.iter()
							.filter(|chunk| chunk.read().unwrap().chunk().owner == user_claims.user)
							.count()
							.into(),
					);
					user_o.insert(
						"notes_owned_public".into(),
						chunks
							.iter()
							.filter(|chunk| {
								let chunk = chunk.read().unwrap();
								chunk.chunk().owner == user_claims.user && chunk.has_access(&"public".into())
							})
							.count()
							.into(),
					);
					user = json!(user_o);
				}
				return reply((&user).into());
			}

			error!("Message {m:?} unknown");
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

		info!("Triggered '{}' to '{}'", &message.message.resource, user);

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
			_ = shutdown_rx.changed() => {
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
