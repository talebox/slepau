use chrono::Local;
use common::{
	proquint::Proquint,
	samn::{log_info, log_limbs},
	socket::{ResourceMessage, SocketMessage},
	utils::LockedAtomic,
};
use core::str;
use embedded_hal::digital::InputPin;
use linux_embedded_hal::{CdevPin, SpidevDevice};
use samn_common::{
	cc1101::{Cc1101, MachineState},
	node::{Command, Message, MessageData, Response},
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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
	// We set this here because calibration is performed from Iddle to RX / TX
	let mut last_calibration = Instant::now();
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
			Ok(v) => match v {
				Ok((payload, is_nrf)) => {
					if !payload.len_is_valid() {
						log::error!(
							"{} packet length {} isn't valid, discarding & flushing rx fifo",
							if is_nrf { "nrf24" } else { "cc1101" },
							payload.len()
						);
						if is_nrf {
							nrf24.flush_rx().unwrap();
						} else {
							cc1101.flush_rx().unwrap();
						}
						nb::Result::Err(nb::Error::WouldBlock)
					} else {
						Ok((payload, is_nrf))
					}
				}
				Err(e) => Err(e),
			},
			Err(delay) => {
				log::error!("Receiving took too long {:?}", delay);
				nb::Result::Err(nb::Error::WouldBlock)
			}
		}
	}

	async fn transmit<E: Debug, R: Radio<E>>(radio: &mut R, payload: &Payload) -> bool {
		// let spin_sleeper = spin_sleep::SpinSleeper::new(100_000)
		// .with_spin_strategy(spin_sleep::SpinStrategy::SpinLoopHint);
		

		radio.transmit_start(payload, &mut linux_embedded_hal::Delay).unwrap(); // This enables ce
		// This is to wait for cc1101 to switch to TX mode
		// Because the polling only asks wether it's in Iddle
		// If this wasn't here transmit_start would send command probe and radio
		// could still be in Idle when we poll it.
		// tokio::time::sleep(Duration::from_micros(30)).await;
		// spin_sleeper.sleep_ns(30_000);

		let mut interval = tokio::time::interval(Duration::from_micros(100));
		interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
		interval.tick().await; // Let first instant tick happen
		loop {
			interval.tick().await; // This first await should be >= 100us long
			match radio.transmit_poll() {
				nb::Result::Ok(v) => {return v;},
				nb::Result::Err(e) => {
					match e {
						nb::Error::Other(e) => {
							log::error!("{:?}", e);
							return false;
						}
						nb::Error::WouldBlock => {
							// Keep polling
						}
					}
				}
			}
		}
	}
	async fn transmit_any<E0: Debug, R0: Radio<E0>, E1: Debug, R1: Radio<E1>>(
		nrf24: &mut R0,
		cc1101: &mut R1,
		message: &Message,
		address: u16,
		is_nrf: bool,
		new_wire_format: bool,
	) {
		let payload = {
			let mut packet = [0u8; 32];
			let packet_l;
			if new_wire_format {
				packet_l = message.serialize_to_bytes(&mut packet).unwrap();
			} else {
				packet_l = postcard::to_slice(&message, &mut packet).unwrap().len();
			}
			Payload::new_with_addr_from_array(packet, packet_l, address, addr_to_rx_pipe(address))
		};

		match timeout(Duration::from_millis(100), async {
			if is_nrf {
				transmit(nrf24, &payload).await
			} else {
				transmit(cc1101, &payload).await
			}
		})
		.await
		{
			Ok(success) => {
				if is_nrf {
					nrf24.to_rx().unwrap();
				} else {
					cc1101.to_rx().unwrap();
				}

				println!(
					"{} {} bytes{}, {:?}",
					if success { "Sent" } else { "Failed sending" },
					payload.len(),
					if new_wire_format {" with new wire format"}else{""},
					message
				);
			}
			Err(delay) => {
				if is_nrf {
					nrf24.to_idle().unwrap();
					nrf24.flush_tx().unwrap();
					nrf24.flush_rx().unwrap();
					nrf24.to_rx().unwrap();
				} else {
					cc1101.to_idle().unwrap();
					cc1101.flush_tx().unwrap();
					cc1101.flush_rx().unwrap();
					cc1101.to_rx().unwrap();
				}

				log::error!(
					"Sending with {} took too long {:?}, flushed fifos & switched to rx just in case",
					if is_nrf { "nrf24" } else { "cc1101" },
					delay
				);
			}
		};
	}
	let send_commands_changed = |db: &db::DB| {
		tx_resource
			.send(SocketMessage::from(("commands", &db.commands())).into())
			.ok();
	};

	async fn poll_pin(pin: &mut CdevPin, state: bool) {
		loop {
			if if state {
				pin.is_high().unwrap()
			} else {
				pin.is_low().unwrap()
			} {
				break;
			}
			tokio::time::sleep(Duration::from_micros(400)).await;
		}
	}

	/// Checks radio state, flushes buffers and performs calibration
	async fn cc1101_checkpoint(cc1101: &mut Cc1101<SpidevDevice>) {
		let marc_state = cc1101.get_marc_state().unwrap();
		if marc_state != MachineState::RX.value() {
			log::error!("cc1101 not in rx state, instead it's in: {marc_state}");
			cc1101.flush_rx().unwrap();
			cc1101.flush_tx().unwrap();
		}
		cc1101.to_idle().unwrap();
		cc1101.to_rx().unwrap();
	}
	// Amount of times checkpoint info log will be printed
	let mut cc1101_checkpoint_n = 30;

	loop {
		let mut pin_awake = false;
		tokio::select! {
			Some(message) = radio_rx.recv() => {
				let non_instant_command = db.write().unwrap().queue_command(message);
				if non_instant_command {
					send_commands_changed(&db.read().unwrap());
				}
			}
			// Make polling functions for IRQ pins
			_ = poll_pin(&mut g2, true) => {pin_awake=true;}
			_ = poll_pin(&mut irq_pin, false) => {pin_awake=true;}
			_ = time::sleep(Duration::from_secs(5)) => {}
			_ = shutdown_rx.changed() => {
				break;
			}
		}

		let rx_start = Instant::now();
		while let Ok((payload, is_nrf)) = receive_any(&mut nrf24, &mut cc1101, &mut irq_pin, &mut g2).await {
			let rx_end = Instant::now();
			if let Ok((message, new_wire_format)) = Message::deserialize_from_bytes(&payload.data())
				.map(|v| (v.0, true))
				.or_else(|_| postcard::from_bytes::<Message>(&payload.data()).map(|v| (v, false)))
			{
				let id_node_db = payload
					.address()
					.and_then(|address| db.read().unwrap().addresses.get_by_right(&address).copied());

				match (message.clone(), id_node_db, payload.address()) {
					(Message::SearchingNetwork(node_id), _, Some(payload_address)) => {
						// Only use node_id
						let node_addr = db.write().unwrap().issue_address(node_id, is_nrf);
						// Send message
						let message = Message::Network(node_id, node_addr);
						// let tx_start = Instant::now();
						transmit_any(
							&mut nrf24,
							&mut cc1101,
							&message,
							payload_address,
							is_nrf,
							new_wire_format,
						)
						.await;
						// println!(
						// 	"Receive -> transmit {:?}, transmit delay {:?}",
						// 	rx_start - rx_end,
						// 	Instant::now() - tx_start
						// );
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
							transmit_any(
								&mut nrf24,
								&mut cc1101,
								&message,
								payload_address,
								is_nrf,
								new_wire_format,
							)
							.await;
							// println!(
							// 	"Receive -> transmit {:?}, transmit delay {:?}",
							// 	rx_end - tx_start,
							// 	Instant::now() - tx_start
							// );
						}

						// Log this message
						match &response {
							Response::Info(info) => {
								log_info(id_node_db, info);
								db.write()
									.unwrap()
									.radio_info
									.entry(id_node_db)
									.and_modify(|(_, _, _, _info)| {
										*_info = Some(info.clone());
									})
									.or_insert((Instant::now(), get_nanos(), 0, Some(info.clone())));
								// db.write()
								// 	.unwrap()
								// 	.limbs_cache
								// 	.entry(id_node_db)
								// 	.and_modify(|(_info, _)| *_info = Some(info.clone()));
							}
							Response::Limbs(limbs) => {
								let limbs = limbs
									.iter()
									.filter_map(|l| if let Some(l) = l { Some(l.clone()) } else { None })
									.collect::<Vec<_>>();
								log_limbs(id_node_db, &limbs);
								db.write()
									.unwrap()
									.limbs_cache
									.entry(id_node_db)
									.and_modify(|_limbs| *_limbs = limbs);
							}
							Response::Heartbeat(seconds) => {
								db.write()
									.unwrap()
									.radio_info
									.entry(id_node_db)
									.and_modify(|(_, _, seconds_, _)| {
										*seconds_ = *seconds;
									})
									.or_insert((Instant::now(), get_nanos(), *seconds, None));
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
							.radio_info
							.entry(id_node_db)
							.and_modify(|(instant, last, _, _)| {
								*instant = Instant::now();
								*last = get_nanos();
							})
							.or_insert((Instant::now(), get_nanos(), 0, None));
					}
					(Message::DebugMessage(_, message), _, _) => {
						if let Ok(string) = str::from_utf8(&message) {
							println!("Decoded message: '{string}'");
						}
					}
					_ => {}
				}
				println!(
					"{} Payload len {}, pin {}, addr {:?}, {:?}",
					Local::now().format("%a %b %e %T"),
					payload.len(),
					pin_awake,
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

		// Send out instant message
		let instant_command = db.write().unwrap().get_next_instant_command_message();
		if let Some((command_id, node_address, command_message)) = instant_command {
			// Send command
			let message = Message::Message(MessageData::Command {
				id: command_id,
				command: command_message.command,
			});

			let tx_start = Instant::now();
			transmit_any(&mut nrf24, &mut cc1101, &message, node_address, true, true).await;
			println!("Transmit delay {:?}", tx_start.elapsed());
		}

		if (Instant::now() - last_calibration).as_secs() > 30 {
			last_calibration = Instant::now();
			cc1101_checkpoint(&mut cc1101).await;
			if cc1101_checkpoint_n > 0 {
				cc1101_checkpoint_n -= 1;
				println!(
					"cc1101 checkpoint took {:?}, will log this {cc1101_checkpoint_n} more times.",
					Instant::now() - last_calibration
				);
			}
		}
	}
	nrf24.to_idle().unwrap();
	cc1101.to_idle().unwrap();
	println!("radios shut down.");
}
