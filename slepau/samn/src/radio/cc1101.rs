use embedded_hal::delay::DelayNs;
use linux_embedded_hal::{
	gpio_cdev::{Chip, LineRequestFlags},
	spidev::SpidevOptions,
	CdevPin, CdevPinError, SpidevDevice,
};
use samn_common::{cc1101::Cc1101, radio::Radio};

use crate::db::HQ_PIPES;

pub fn init(chip: &mut Chip) -> (Cc1101<SpidevDevice>, CdevPin) {
	let mut spi = linux_embedded_hal::SpidevDevice::open("/dev/spidev0.1").unwrap();
	spi
		.0
		.configure(&SpidevOptions {
			max_speed_hz: Some(6_500_000),
			..Default::default()
		})
		.unwrap();

	let g2 = linux_embedded_hal::CdevPin::new(
		chip
			.get_line(6)
			.unwrap()
			.request(LineRequestFlags::INPUT, 0, "cc1101_g2")
			.unwrap(),
	).unwrap();
    // let g0 = linux_embedded_hal::CdevPin::new(
	// 	chip
	// 		.get_line(12)
	// 		.unwrap()
	// 		.request(LineRequestFlags::INPUT, 0, "cc1101_g0")
	// 		.unwrap(),
	// ).unwrap();
    let mut delay = linux_embedded_hal::Delay;
	let mut cc1101 = Cc1101::new(spi).unwrap();
    cc1101.reset().unwrap();
    delay.delay_ms(1);
	cc1101.configure();
	cc1101.set_rx_filter(&HQ_PIPES).unwrap();

	println!("Initalized the cc1101");

	(cc1101, g2)
}
