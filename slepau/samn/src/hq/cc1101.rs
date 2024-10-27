use linux_embedded_hal::{
	gpio_cdev::{Chip, LineRequestFlags},
	spidev::SpidevOptions,
	CdevPin, SpidevDevice,
};
use rppal::gpio::InputPin;
use samn_common::{cc1101::Cc1101, radio::Radio};

use crate::db::HQ_PIPES;

pub fn init() -> (Cc1101<SpidevDevice>, InputPin) {
	let mut spi = linux_embedded_hal::SpidevDevice::open("/dev/spidev0.1").unwrap();
	spi
		.0
		.configure(&SpidevOptions {
			max_speed_hz: Some(6_500_000),
			..Default::default()
		})
		.unwrap();

	let g2 = rppal::gpio::Gpio::new().unwrap().get(23).unwrap().into_input();
  // let g0 = linux_embedded_hal::CdevPin::new(
	// 	chip
	// 		.get_line(12)
	// 		.unwrap()
	// 		.request(LineRequestFlags::INPUT, 0, "cc1101_g0")
	// 		.unwrap(),
	// ).unwrap();
	let mut cc1101 = Cc1101::new(spi).unwrap();
	cc1101.init(&mut linux_embedded_hal::Delay).unwrap();
	cc1101.set_rx_filter(&HQ_PIPES).unwrap();

	println!("Initalized the cc1101");

	(cc1101, g2)
}
