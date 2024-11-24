use embedded_hal::spi::SpiDevice;
use linux_embedded_hal::{
	gpio_cdev::{Chip, LineRequestFlags},
	spidev::{SpiModeFlags, SpidevOptions},
	CdevPin, SpidevDevice,
};
use samn_common::{nrf24::NRF24L01, radio::Radio};

use crate::db::HQ_PIPES;

pub fn init(chip: &mut Chip) -> (NRF24L01<SpidevDevice, CdevPin>, CdevPin) {
	let mut spi = linux_embedded_hal::SpidevDevice::open("/dev/spidev0.0").unwrap();
	spi
		.0
		.configure(&SpidevOptions {
			max_speed_hz: Some(8_000_000),
			spi_mode: Some(SpiModeFlags::SPI_MODE_0), 
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
	let mut nrf24 = NRF24L01::new(ce_pin, spi);
	nrf24.init(&mut linux_embedded_hal::Delay).unwrap();

	// // Enabling all pipes
	// const PIPES: [bool; 6] = [true;6];
	// nrf24.set_auto_ack_pipes(&PIPES).unwrap();
	// nrf24.set_rx_enabled_pipes(&PIPES).unwrap();
	// nrf24.set_dynamic_payload_pipes(&PIPES).unwrap();

	nrf24.set_rx_filter(&HQ_PIPES).unwrap();
	
	// Not working with more than first pipe open :(
	// let pipes = [true,true, false,false,false,false];
	// nrf24.set_pipes_rx_enable(&pipes).unwrap();
	// nrf24.set_auto_ack(&pipes).unwrap();

	// println!("Initalized the nrf24, connected: {}", nrf24.is_connected().unwrap());
	println!("After configuration, here's the register values V");
	{
		let mut a = |i| {
			let w = [i; 1];
			let mut r = [0x00; 6];
			nrf24.spi.transfer(&mut r, &w).unwrap();

			println!(
				"{:02x} -> {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
				w[0],
				r[0],
				r[1],
				r[2],
				r[3],
				r[4],
				r[5]
			);
		};

		for i in 0..=0x17 {
			a(i);
		}
		a(0x1C);
		a(0x1D);
	}

	(nrf24, irq_pin)
}
