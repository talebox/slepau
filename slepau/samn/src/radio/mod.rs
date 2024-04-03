use common::{
	proquint::Proquint,
	samn::{log_info, log_limbs},
	utils::LockedAtomic,
};
use samn_common::{
	node::{Command, Message, MessageData, Response},
	nrf24::Device,
	radio::*,
};
use serde::{Deserialize, Serialize};
use std::{
	collections::{HashMap, LinkedList},
	fmt::Debug,
	time::{Duration, Instant},
};
use tokio::{
	sync::{mpsc, oneshot, watch},
	time,
};

use crate::db;
mod cc1101;
mod nrf24;

#[derive(Deserialize, Debug)]
pub struct CommandMessage {
	for_id: Proquint<u16>,
	command: Command,
}

pub type RadioSyncType = (CommandMessage, Option<oneshot::Sender<Response>>);

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

	println!("Receiving...");
	nrf24.rx().unwrap();
	cc1101.to_rx().unwrap();

	let mut command_messages = LinkedList::<(CommandMessage, Option<oneshot::Sender<Response>>)>::new();
	let mut response_callbacks = LinkedList::<(u8, oneshot::Sender<Response>)>::new();

	loop {
		if let Ok((payload, is_nrf)) = nrf24
			.receive_(&mut irq_pin)
			.map(|v| (v, true))
			.or_else(|_| cc1101.receive_(&mut g2).map(|v| (v, false)))
		{
			if let Ok(message) = postcard::from_bytes::<Message>(&payload.data()) {
				println!("{:?}", &message);
				let id_node = message.id;
				if let MessageData::Response {
					id: id_command,
					response,
				} = message.data
				{
					// Check if we haven't received a packet from this node in more than 25 seconds
					// Check that the message queue for this node isn't more than 6
					// And if so queue Info + Limbs commands
					if db
						.read()
						.unwrap()
						.heartbeats
						.get(&id_node)
						.map(|(last, _)| (Instant::now() - *last).as_secs() > 25)
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
						let packet = postcard::to_vec::<_, 32>(&Message {
							id: id_node,
							data: MessageData::Command {
								id: command_id,
								command: message.command,
							},
						})
						.unwrap();
						// Increment the command id
						{
							let mut db = db.write().unwrap();
							db.command_id = db.command_id.wrapping_add(1);
						}

						println!("Sending {} bytes", packet.len());
						// Send command
						if is_nrf {
							nrf24.ce_disable();
							nrf24.transmit_(&Payload::new(&packet)).unwrap();
							nrf24.rx().unwrap();
						} else {
							cc1101.to_idle().unwrap();
							cc1101.transmit_(&Payload::new(&packet)).unwrap();
							cc1101.to_rx().unwrap();
						}
					}

					// Log this message
					match &response {
						Response::Info(info) => {
							log_info(id_node, info);
						}
						Response::Limbs(limbs) => {
							log_limbs(id_node, limbs);
						}
						Response::Heartbeat(seconds) => {
							db.write()
								.unwrap()
								.heartbeats
								.entry(id_node)
								.and_modify(|(_, seconds_)| {
									*seconds_ = *seconds;
								})
								.or_insert((Instant::now(), *seconds));
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
						.and_modify(|(instant, _)| {
							*instant = Instant::now();
						})
						.or_insert((Instant::now(), 0u32));
				}
			} else {
				// let text = std::str::from_utf8(&bytes).unwrap();
				println!(
					"Couldn't deserialize, received {} bytes: {:?}",
					payload.len(),
					&payload.data()
				);
			}
		}

		tokio::select! {
			message = radio_rx.recv() => {
				if let Some(message) = message{command_messages.push_back(message);}
			}
			_ = time::sleep(Duration::from_millis(10)) => {}
			_ = shutdown_rx.changed() => {
				break;
			}
		}
	}
	nrf24.ce_disable();
	cc1101.to_idle().unwrap();
	println!("radios shut down.");
}
