use common::{
	proquint::Proquint,
	samn::{log_info, log_limbs},
	utils::LockedAtomic,
};
use embedded_hal::digital::InputPin;
use linux_embedded_hal::CdevPin;
use log::info;
use samn_common::{
	node::{Command, Limb, Message, MessageData, Response},
	nrf24::Device,
	radio::*,
};
use serde::{Deserialize, Serialize};
use std::{
	collections::{HashMap, LinkedList},
	fmt::Debug,
	time::{Duration, Instant, SystemTime},
};
use tokio::{
	sync::{mpsc, oneshot, watch},
	time,
};

use crate::db::{self, HQ_PIPES};
mod cc1101;
mod nrf24;

#[derive(Deserialize, Debug)]
pub struct CommandMessage {
	for_id: Proquint<u32>,
	command: Command,
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
use std::task::{Context, Poll};

fn poll_irq_pin(_cx: &mut Context<'_>) -> Poll<String> {
	Poll::Ready("Hello, World!".into())
}

// not a test anymore, it works :)
pub async fn radio_service(
	db: LockedAtomic<db::DB>,
	mut shutdown_rx: watch::Receiver<()>,
	mut radio_rx: mpsc::Receiver<RadioSyncType>,
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

	let mut command_messages = LinkedList::<(CommandMessage, Option<oneshot::Sender<Response>>)>::new();
	let mut response_callbacks = LinkedList::<(u8, oneshot::Sender<Response>)>::new();

	fn receive_any<E0: Debug, R0: Radio<E0>, E1: Debug, R1: Radio<E1>>(
		nrf24: &mut R0,
		cc1101: &mut R1,
		nrf24_pin: &mut CdevPin,
		cc1101_pin: &mut CdevPin,
	) -> nb::Result<(Payload, bool), E1> {
		nrf24
			.receive(nrf24_pin, None)
			.map(|v| (v, true))
			.or_else(|_| cc1101.receive(cc1101_pin, None).map(|v| (v, true)))
	}

	fn transmit<E: Debug, R: Radio<E>>(radio: &mut R, payload: &Payload) {
		radio.transmit(payload).unwrap();
	}
	fn transmit_any<E0: Debug, R0: Radio<E0>, E1: Debug, R1: Radio<E1>>(
		nrf24: &mut R0,
		cc1101: &mut R1,
		message: &Message,
		address: u16,
		is_nrf: bool,
	) {
		let packet = postcard::to_vec::<_, 32>(&message).unwrap();
		let payload = Payload::new_with_addr(&packet, address, addr_to_rx_pipe(address));
		if is_nrf {
			transmit(nrf24, &payload);
			nrf24.to_rx().unwrap();
		} else {
			transmit(cc1101, &payload);
			cc1101.to_rx().unwrap();
		}
		println!("Sent {} bytes {:?}", packet.len(), message);
	}

	let mut before_receive;

	loop {
		tokio::select! {
			message = radio_rx.recv() => {
				if let Some(message) = message{command_messages.push_back(message);}
			}
			// Make polling functions for IRQ pins
			_ = poll_fn(|_| {
				if g2.is_high().unwrap() {Poll::Ready(())} else {Poll::Pending}
			}) => {
				// info!("G2 trigger");
			}
			_ = poll_fn(|_| {
				if irq_pin.is_low().unwrap() {
					Poll::Ready(())
				} else {Poll::Pending}
			}) => {
				// info!("IRQ trigger");
			}
			_ = time::sleep(Duration::from_millis(10)) => {}
			_ = shutdown_rx.changed() => {
				break;
			}
		}
		before_receive = Instant::now();

		while let Ok((payload, is_nrf)) = receive_any(&mut nrf24, &mut cc1101, &mut irq_pin, &mut g2) {
			if let Ok(message) = postcard::from_bytes::<Message>(&payload.data()) {
				let id_node = payload
					.address()
					.and_then(|address| db.read().unwrap().addresses.get_by_right(&address).cloned());
				if id_node.is_none() {
					// info!(
					// 	"No matching id found for address {:?}, will prob issue a new one.",
					// 	payload.address()
					// )
				}

				match (message.clone(), id_node, payload.address()) {
					(Message::SearchingNetwork(node_id), _, Some(payload_address)) => {
						// Only use node_id
						let mut db = db.write().unwrap();
						// if !db.addresses.contains_left(&node_id) {
						// 	let mut new_address: u16 = random();
						// 	while db.addresses.contains_right(&new_address) {
						// 		new_address = random();
						// 	}
						// 	db.addresses.insert(node_id, new_address);
						// }
						// let node_address = db.addresses.get_by_left(&node_id).cloned().unwrap();
						let node_addr = db.issue_address(node_id, is_nrf);
						// Send message
						let message = Message::Network(node_id, node_addr);
						let transmit_instant = Instant::now();
						transmit_any(&mut nrf24, &mut cc1101, &message, payload_address, is_nrf);
						println!(
							"Receive -> transmit {:?}, transmit delay {:?}",
							transmit_instant - before_receive,
							Instant::now() - transmit_instant
						);
					}
					(
						Message::Message(MessageData::Response {
							id: id_command,
							response,
						}),
						Some(id_node),
						Some(payload_address),
					) => {
						// Check if we haven't received a packet from this node in more than 25 seconds
						// Check that the message queue for this node isn't more than 6
						// And if so queue Info + Limbs commands
						if db
							.read()
							.unwrap()
							.heartbeats
							.get(&id_node)
							.map(|(last, _, _, interval)| (Instant::now() - *last).as_secs() > (*interval * 3).into())
							.unwrap_or(true)
							&& command_messages
								.iter()
								.filter(|(m, _)| m.for_id.inner() == id_node)
								.count() < 2
						{
							command_messages.push_back((
								CommandMessage {
									for_id: id_node.into(),
									command: Command::Info,
								},
								None,
							));
							command_messages.push_back((
								CommandMessage {
									for_id: id_node.into(),
									command: Command::Limbs,
								},
								None,
							));
						}
						// println!("hearbeats: {:?}\ncommand_messages: {:?}", heartbeats, command_messages);
						// Send a command to the node if one is available
						if let Some(i) = command_messages.iter().position(|(m, _)| m.for_id.inner() == id_node) {
							let (message, callback) = command_messages.remove(i);
							let command_id = db.read().unwrap().command_id;
							// Add callback to another array with an id
							// so we know what to call later if we receive a response
							if let Some(callback) = callback {
								response_callbacks.push_back((command_id, callback));
							}

							// Send command
							let message = Message::Message(MessageData::Command {
								id: command_id,
								command: message.command,
							});
							// Increment the command id
							{
								let mut db = db.write().unwrap();
								db.command_id = db.command_id.wrapping_add(1);
							}

							let transmit_instant = Instant::now();
							transmit_any(&mut nrf24, &mut cc1101, &message, payload_address, is_nrf);
							println!(
								"Receive -> transmit {:?}, transmit delay {:?}",
								transmit_instant - before_receive,
								Instant::now() - transmit_instant
							);
						}

						// Log this message
						match &response {
							Response::Info(info) => {
								log_info(id_node, info);
								db.write()
									.unwrap()
									.heartbeats
									.entry(id_node)
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
								log_limbs(id_node, &limbs);
							}
							Response::Heartbeat(seconds) => {
								db.write()
									.unwrap()
									.heartbeats
									.entry(id_node)
									.and_modify(|(_, _, seconds_, _)| {
										*seconds_ = *seconds;
									})
									.or_insert((Instant::now(), get_nanos(), *seconds, 10));
							}
							_ => {}
						}
						// Call back anything that needed this command
						if let Some(id_command) = id_command {
							if let Some(i) = response_callbacks
								.iter()
								.position(|(id_command_, _)| *id_command_ == id_command)
							{
								let (_, callback) = response_callbacks.remove(i);
								callback.send(response).ok();
							}
							// Remove all callbacks that are closed
							response_callbacks.retain(|(_, callback)| !callback.is_closed());
						}
						// Update the heartbeat
						db.write()
							.unwrap()
							.heartbeats
							.entry(id_node)
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
