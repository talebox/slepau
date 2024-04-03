use linux_embedded_hal::{
	gpio_cdev::{Chip, LineRequestFlags},
	spidev::SpidevOptions,
	CdevPin, CdevPinError, SpidevDevice,
};
use samn_common::nrf24::NRF24L01;

pub fn init(chip: &mut Chip) -> (NRF24L01<CdevPinError, CdevPin, SpidevDevice>, CdevPin) {
	let mut spi = linux_embedded_hal::SpidevDevice::open("/dev/spidev0.0").unwrap();
	spi
		.0
		.configure(&SpidevOptions {
			max_speed_hz: Some(8_000_000),
			..Default::default()
		})
		.unwrap();

	let ce_pin = linux_embedded_hal::CdevPin::new(
		chip
			.get_line(25)
			.unwrap()
			.request(LineRequestFlags::OUTPUT, 0, "nrf24_ce")
			.unwrap(),
	)
	.unwrap();
	let irq_pin = linux_embedded_hal::CdevPin::new(
		chip
			.get_line(24)
			.unwrap()
			.request(LineRequestFlags::INPUT, 0, "nrf24_irq")
			.unwrap(),
	)
	.unwrap();
	let mut nrf24 = NRF24L01::new(ce_pin, spi).unwrap();
	nrf24.configure().unwrap();

	println!("Initalized the nrf24, connected: {}", nrf24.is_connected().unwrap());

	(nrf24, irq_pin)
}
