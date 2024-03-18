use linux_embedded_hal::{gpio_cdev::LineRequestFlags, spidev::SpidevOptions};
use samn_common::radio::nrf24::{self, Device};
use std::time::Duration;
use tokio::{sync::watch,time};

// pub type MessageSender = broadcast::Sender<ResourceMessage>;

// THIS IS PURELY A TEST
pub async fn radio_service(mut shutdown_rx: watch::Receiver<()>) {
	let mut spi = linux_embedded_hal::SpidevDevice::open("/dev/spidev0.0").unwrap();
	spi.0.configure(&SpidevOptions {
		max_speed_hz: Some(8_000_000),
		..Default::default()
	}).unwrap();
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
	loop {

		if nrf24.can_read().unwrap().is_some() {
			let bytes = nrf24.read().unwrap();
			let text = std::str::from_utf8(&bytes).unwrap();
			println!("Received: {}",text); 
		}
		
		tokio::select! {
			_ = time::sleep(Duration::from_millis(5)) => {
				continue;
			}
			_ = shutdown_rx.changed() => {
				break;
			}
		}
	}
	nrf24.ce_disable();
}
