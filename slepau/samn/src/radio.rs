use common::samn::{log_info, log_limbs};
use linux_embedded_hal::{gpio_cdev::LineRequestFlags, spidev::SpidevOptions};
use samn_common::{
	node::{Command, Message, MessageData, Response},
	radio::nrf24::{self, Device},
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

#[derive(Deserialize, Debug)]
pub struct CommandMessage {
	for_id: u16,
	command: Command,
}

pub type RadioSyncType = (CommandMessage, Option<oneshot::Sender<Response>>);

// not a test anymore, it works :)
pub async fn radio_service(mut shutdown_rx: watch::Receiver<()>, mut radio_rx: mpsc::Receiver<RadioSyncType>) {
	if std::env::var("RADIO").is_err() {
		println!("Radio is off, if you want it enabled, set RADIO environment.");
		return;
	}

	let mut spi = linux_embedded_hal::SpidevDevice::open("/dev/spidev0.0").unwrap();
	spi
		.0
		.configure(&SpidevOptions {
			max_speed_hz: Some(8_000_000),
			..Default::default()
		})
		.unwrap();
	let mut chip = linux_embedded_hal::gpio_cdev::Chip::new("/dev/gpiochip0").unwrap();

	let line = chip
		.get_line(25)
		.unwrap()
		.request(LineRequestFlags::OUTPUT, 0, "nrf24")
		.unwrap();
	let ce_pin = linux_embedded_hal::CdevPin::new(line).unwrap();
	let mut nrf24 = nrf24::NRF24L01::new(ce_pin, spi).unwrap();

	nrf24::init(&mut nrf24);
	println!("Initalized the nrf24");
	println!("Radio is connected: {}", nrf24.is_connected().unwrap());

	println!("Receiving...");
	nrf24.rx().unwrap();
	let mut heartbeats: HashMap<u16, (Instant, u32)> = HashMap::new();
	let mut command_messages = LinkedList::<(CommandMessage, Option<oneshot::Sender<Response>>)>::new();
	let mut response_callbacks = LinkedList::<(u8, oneshot::Sender<Response>)>::new();
	let mut command_id: u8 = 0;
	loop {
		if let Ok(bytes) = nrf24.receive() {
			if let Ok(message) = postcard::from_bytes::<Message>(&bytes) {
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
					if heartbeats
						.get(&id_node)
						.map(|(last, _)| (Instant::now() - *last).as_secs() > 25)
						.unwrap_or(true)
						&& command_messages.iter().filter(|(m, _)| m.for_id == id_node).count() < 2
					{
						command_messages.push_back((
							CommandMessage {
								for_id: id_node,
								command: Command::Info,
							},
							None,
						));
						command_messages.push_back((
							CommandMessage {
								for_id: id_node,
								command: Command::Limbs,
							},
							None,
						));
					}
					// println!("hearbeats: {:?}\ncommand_messages: {:?}", heartbeats, command_messages);
					// Send a command to the node if one is available
					if let Some(i) = command_messages.iter().position(|(m, _)| m.for_id == id_node) {
						let (message, callback) = command_messages.remove(i);

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
						command_id = command_id.wrapping_add(1);
						println!("Sending {} bytes", packet.len());
						// Send command
						nrf24.ce_disable();
						nrf24.send(&packet).unwrap();
						nrf24.rx().unwrap();
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
							heartbeats
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
					heartbeats
						.entry(id_node)
						.and_modify(|(instant, _)| {
							*instant = Instant::now();
						})
						.or_insert((Instant::now(), 0u32));
				}
			} else {
				// let text = std::str::from_utf8(&bytes).unwrap();
				println!("Couldn't deserialize, received {} bytes: {:?}", bytes.len(), &bytes);
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
	println!("nrf24 shut down.");
}
