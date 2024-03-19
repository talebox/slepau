use linux_embedded_hal::{gpio_cdev::LineRequestFlags, spidev::SpidevOptions};
use samn_common::{
	node::{Command, MessageData, Response},
	radio::nrf24::{self, Device},
};
use serde::{Deserialize, Serialize};
use std::{collections::LinkedList, fmt::Debug, time::Duration};
use tokio::{
	sync::{mpsc, oneshot, watch},
	time,
};

#[derive(Deserialize)]
pub struct CommandMessage {
	for_id: u16,
	command: Command,
}

pub type RadioSyncType = (CommandMessage,Option<oneshot::Sender<Response>>);

// THIS IS PURELY A TEST
pub async fn radio_service(mut shutdown_rx: watch::Receiver<()>, mut radio_rx: mpsc::Receiver<RadioSyncType>) {
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
	let mut command_messages = LinkedList::<(CommandMessage, Option<oneshot::Sender<Response>>)>::new();
	let mut response_callbacks = LinkedList::<(u8, oneshot::Sender<Response>)>::new();
	let mut command_id: u8 = 0;
	loop {
		if let Ok(bytes) = nrf24.receive() {
			let mut try_sending_command = |id:u16| {
				if let Some(i) = command_messages.iter().position(|(m,_)| m.for_id == id) {
					let (message,callback) = command_messages.remove(i);
					let c_id = command_id;
					command_id =  command_id.wrapping_add(1);
					// Add callback to another array with an id
					// so we know what to call later if we receive a response
					if let Some(callback) = callback {
						response_callbacks.push_back((c_id, callback));
					}
					// Send command
					nrf24.ce_disable();
					nrf24.send(
						&postcard::to_vec::<_, 32>(&MessageData::Command {
							id: c_id,
							command: message.command,
						})
						.unwrap(),
					).unwrap();
					nrf24.rx().unwrap();
				}
			};
			if let Ok(message) = postcard::from_bytes::<MessageData>(&bytes) {
				println!("{:?}", &message);
				match message {
					MessageData::SensorData { id, data } => {
						try_sending_command(id);
					}
					MessageData::Response { id, id_c, response } => {
						try_sending_command(id);
						if let Some(i) = response_callbacks.iter().position(|(r_id,_)| *r_id == id_c) {
							let (_,callback) = response_callbacks.remove(i);
							callback.send(response).ok();
						}
					}
					_ => {}
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
			_ = time::sleep(Duration::from_millis(10)) => {
				continue;
			}
			_ = shutdown_rx.changed() => {
				break;
			}
		}
	}
	nrf24.ce_disable();
}
