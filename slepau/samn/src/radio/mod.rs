use common::{
	proquint::Proquint,
	samn::{log_info, log_limbs},
	socket::{ResourceMessage, SocketMessage},
	utils::LockedAtomic,
};
use embedded_hal::digital::InputPin;
use linux_embedded_hal::CdevPin;
use log::info;
use samn_common::{
	node::{Command, Message, MessageData, Response},
	nrf24::Device,
	radio::*,
};
use serde::{Deserialize, Serialize};
use std::{
	fmt::Debug,
	time::{Duration, Instant, SystemTime},
};
use tokio::{
	sync::{broadcast, mpsc, oneshot, watch},
	time::{self, timeout},
};

use crate::db::{self};
mod cc1101;
mod nrf24;

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandMessage {
	pub for_id: Proquint<u32>,
	pub command: Command,
}

fn get_nanos() -> u64 {
	SystemTime::now()
		.duration_since(SystemTime::UNIX_EPOCH)
		.unwrap()
		.as_nanos()
		.try_into()
		.unwrap()
}

pub type RadioSyncType = (CommandMessage, Option<oneshot::Sender<Response>>);

use core::future::poll_fn;
use std::task::Poll;

// not a test anymore, it works :)
pub async fn radio_service(
	db: LockedAtomic<db::DB>,
	mut shutdown_rx: watch::Receiver<()>,
	mut radio_rx: mpsc::Receiver<RadioSyncType>,
	tx_resource: broadcast::Sender<ResourceMessage>,
) {
	if std::env::var("RADIO").is_err() {
		println!("Radio is off, if you want it enabled, set RADIO environment.");
		return;
	}

	let mut chip = linux_embedded_hal::gpio_cdev::Chip::new("/dev/gpiochip0").unwrap();

	let (mut nrf24, mut irq_pin) = nrf24::init(&mut chip);
	let (mut cc1101, mut g2) = cc1101::init(&mut chip);

	nrf24.to_rx().unwrap();
	cc1101.to_rx().unwrap();
	println!("Receiving...");

	async fn receive_any<E0: Debug, R0: Radio<E0>, E1: Debug, R1: Radio<E1>>(
		nrf24: &mut R0,
		cc1101: &mut R1,
		nrf24_pin: &mut CdevPin,
		cc1101_pin: &mut CdevPin,
	) -> nb::Result<(Payload, bool), E1> {
		match timeout(Duration::from_millis(50), async {
			nrf24
				.receive(nrf24_pin, None)
				.map(|v| (v, true))
				.or_else(|_| cc1101.receive(cc1101_pin, None).map(|v| (v, false)))
		})
		.await
		{
			Ok(v) => v,
			Err(delay) => {
				log::error!("Receiving took too long {:?}", delay);
				nb::Result::Err(nb::Error::WouldBlock)
			}
		}
	}

	fn transmit<E: Debug, R: Radio<E>>(radio: &mut R, payload: &Payload) {
		radio.transmit(payload).unwrap();
	}
	async fn transmit_any<E0: Debug, R0: Radio<E0>, E1: Debug, R1: Radio<E1>>(
		nrf24: &mut R0,
		cc1101: &mut R1,
		message: &Message,
		address: u16,
		is_nrf: bool,
	) {
		let packet = postcard::to_vec::<_, 32>(&message).unwrap();
		let payload = Payload::new_with_addr(&packet, address, addr_to_rx_pipe(address));
		match timeout(Duration::from_millis(100), async {
			if is_nrf {
				transmit(nrf24, &payload);
				nrf24.to_rx().unwrap();
			} else {
				transmit(cc1101, &payload);
				cc1101.to_rx().unwrap();
			}
		})
		.await
		{
			Ok(_) => {
				info!("Sent {} bytes {:?}", packet.len(), message);
			}
			Err(delay) => {
				log::error!(
					"Sending with {} took too long {:?}, switching to rx just in case",
					if is_nrf { "nrf24" } else { "cc1101" },
					delay
				);
				if is_nrf {
					nrf24.to_rx().unwrap();
				} else {
					cc1101.to_rx().unwrap();
				}
			}
		};
	}
	let send_commands_changed = |db: &db::DB| {
		tx_resource
			.send(SocketMessage::from(("commands", &db.commands())).into())
			.ok();
	};

	let mut rx_start;

	loop {
		tokio::select! {
			Some(message) = radio_rx.recv() => {
				db.write().unwrap().command_messages.push_back(message);
				send_commands_changed(&db.read().unwrap());
			}
			// Make polling functions for IRQ pins
			_ = poll_fn(|_| {
				if g2.is_high().unwrap() {Poll::Ready(())} else {Poll::Pending}
			}) => {}
			_ = poll_fn(|_| {
				if irq_pin.is_low().unwrap() {
					Poll::Ready(())
				} else {Poll::Pending}
			}) => {}
			_ = time::sleep(Duration::from_millis(10)) => {}
			_ = shutdown_rx.changed() => {
				break;
			}
		}

		rx_start = Instant::now();
		while let Ok((payload, is_nrf)) = receive_any(&mut nrf24, &mut cc1101, &mut irq_pin, &mut g2).await {
			let rx_end = Instant::now();
			if let Ok(message) = postcard::from_bytes::<Message>(&payload.data()) {
				let id_node_db = payload
					.address()
					.and_then(|address| db.read().unwrap().addresses.get_by_right(&address).copied());

				match (message.clone(), id_node_db, payload.address()) {
					(Message::SearchingNetwork(node_id), _, Some(payload_address)) => {
						// Only use node_id
						let node_addr = db.write().unwrap().issue_address(node_id, is_nrf);
						// Send message
						let message = Message::Network(node_id, node_addr);
						let tx_start = Instant::now();
						transmit_any(&mut nrf24, &mut cc1101, &message, payload_address, is_nrf).await;
						println!(
							"Receive -> transmit {:?}, transmit delay {:?}",
							rx_start - rx_end,
							Instant::now() - tx_start
						);
					}
					(
						Message::Message(MessageData::Response {
							id: id_command,
							response,
						}),
						Some(id_node_db),
						Some(payload_address),
					) => {
						let mut changed_commands = db.write().unwrap().maybe_queue_update(id_node_db);

						// println!("hearbeats: {:?}\ncommand_messages: {:?}", heartbeats, command_messages);
						// Send a command to the node if one is available
						let command_message = db.write().unwrap().get_next_command_message(id_node_db);
						if let Some((command_id, command_message)) = command_message {
							changed_commands = true;

							// Send command
							let message = Message::Message(MessageData::Command {
								id: command_id,
								command: command_message.command,
							});

							let tx_start = Instant::now();
							transmit_any(&mut nrf24, &mut cc1101, &message, payload_address, is_nrf).await;
							println!(
								"Receive -> transmit {:?}, transmit delay {:?}",
								rx_start - rx_end,
								Instant::now() - tx_start
							);
						}

						// Log this message
						match &response {
							Response::Info(info) => {
								log_info(id_node_db, info);
								db.write()
									.unwrap()
									.heartbeats
									.entry(id_node_db)
									.and_modify(|(_, _, _, delay)| {
										*delay = info.heartbeat_interval;
									})
									.or_insert((Instant::now(), get_nanos(), 0, info.heartbeat_interval));
							}
							Response::Limbs(limbs) => {
								let limbs = limbs
									.iter()
									.filter_map(|l| if let Some(l) = l { Some(l.clone()) } else { None })
									.collect::<Vec<_>>();
								log_limbs(id_node_db, &limbs);
							}
							Response::Heartbeat(seconds) => {
								db.write()
									.unwrap()
									.heartbeats
									.entry(id_node_db)
									.and_modify(|(_, _, seconds_, _)| {
										*seconds_ = *seconds;
									})
									.or_insert((Instant::now(), get_nanos(), *seconds, 10));
							}
							_ => {}
						}
						// Call back anything that needed this command
						if let Some(callback) = id_command.and_then(|id| db.write().unwrap().get_response_callback(id)) {
							callback.send(response).ok();
						}
						// Update ui commands
						if changed_commands {
							send_commands_changed(&db.read().unwrap());
						}
						// Update the heartbeat
						db.write()
							.unwrap()
							.heartbeats
							.entry(id_node_db)
							.and_modify(|(instant, last, _, _)| {
								*instant = Instant::now();
								*last = get_nanos();
							})
							.or_insert((Instant::now(), get_nanos(), 0, 10));
					}
					_ => {}
				}
				println!(
					"Payload len {}, addr {:?}, {:?}",
					payload.len(),
					payload.address(),
					&message
				);

				// Notify UI that this node changed
				tx_resource
					.send(ResourceMessage {
						message: SocketMessage {
							resource: format!(
								"nodes{}",
								if let Some(id_node_db) = id_node_db {
									format!("/{}", Proquint::from(id_node_db))
								} else {
									"".into()
								}
							),
							..Default::default()
						},
						..Default::default()
					})
					.ok();
			} else {
				// let text = std::str::from_utf8(&bytes).unwrap();
				println!(
					"Couldn't deserialize, payload received {} bytes: {:?}",
					payload.len(),
					&payload.data()
				);
			}
		}
	}
	nrf24.ce_disable();
	cc1101.to_idle().unwrap();
	println!("radios shut down.");
}
