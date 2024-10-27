use std::{
	thread::sleep,
	time::{Duration, Instant},
};

use log::info;
use rppal::gpio::{Gpio, OutputPin};
use tokio::sync::{mpsc, watch};

#[derive(Clone, Copy, Default)]
pub struct RGB {
	r: u8,
	g: u8,
	b: u8,
}

pub type LEDSyncType = (RGB, Option<LEDState>, u8);

#[derive(Clone, Copy)]
// This holds led states that correspond to functions that turn RGB -> RGB every set intervals of time.
pub enum LEDState {
	CycleColors,
}
struct LedArray {
	// A LOW value on the pin means HIGH in the LedArray because we have a NOT gate implemented with an NPN bjt.
	pin: OutputPin,
	// An array of 8 RGB LEDs
	led_state: [(
		RGB,
		// Does the LED have a pulsing state?
		Option<LEDState>,
	); 8],
	clock_cycles_per_micro: u32,
}





impl LedArray {
	/// Make sure to reculate cycles per micro
	/// before running this :)
	fn delay_nanos(&self, v: u32) {
		// Gonna do no_op busy loop
		let cycles = self.clock_cycles_per_micro * v / 1_000;
		for _ in 0..cycles {
			unsafe {
					core::arch::asm!("nop");
			}
		}

		// Giving up on Instant, takes about 160ns
		// let v = Duration::from_nanos(v);
		// let start = Instant::now();
		// while Instant::now() - start < v {}
	
		// Giving up the thread is not precise whatsoever
		// sleep(Duration::from_nanos(v));
	}
	fn recalculate_cycles_per_micro(&mut self) {
		const cycles:u32 = 10_000;
		let start = Instant::now();
		// Run for 10k cycles
		for _ in 0..cycles {
			unsafe {
					core::arch::asm!("nop");
			}
		}
		let end = Instant::now();
		self.clock_cycles_per_micro = cycles * 1_000 / ((end-start).as_nanos() as u32) ;
	}
	fn maybe_recalculate(&mut self) {
		if self.clock_cycles_per_micro==0 {self.recalculate_cycles_per_micro();}
	}
	fn reset(&mut self) {
		self.recalculate_cycles_per_micro();
		self.pin.set_high(); // go low
		self.delay_nanos(20_000);
		self.pin.set_low();
		self.delay_nanos(100_000); // Flush command >80us high
		self.pin.set_high();
		self.delay_nanos(20_000); // Wait a bit low
	}
	fn bit(&mut self, b: bool) {
		self.maybe_recalculate();
		const PULSE_DURATION: u32 = 1200;
		const HIGH_PULSE: u32 = 600;
		const LOW_PULSE: u32 = 220;
		self.pin.set_low(); // go high
		self.delay_nanos(if b { HIGH_PULSE } else { LOW_PULSE }); // wait at least 220 for low bit, 600 for high
		// while Instant::now() - start < duration_high {} // wait at least 220 for low bit, 600 for high
		self.pin.set_high(); // go low
		self.delay_nanos(if b { PULSE_DURATION - HIGH_PULSE } else {  PULSE_DURATION - LOW_PULSE }); // wait out reset of the pulse low
		// while Instant::now() - start < PULSE_DURATION {} // wait out reset of the pulse low
	}
	fn color(&mut self, color: &RGB) {
		self.maybe_recalculate();
		let c = [false; 24]
			.into_iter()
			.enumerate()
			.map(|(i, _)| {
				let (p, c) = (i % 8, i / 8);
				([color.g, color.r, color.b][c] >> (7 - p)) & 1
			})
			.collect::<Vec<_>>();

		c.iter().for_each(|b| self.bit(*b == 1));
	}
	fn update(&mut self) {
		self.recalculate_cycles_per_micro();
		// Calculate LED state
		for led_n in 0..self.led_state.len() {
			let (c, state) = &mut self.led_state[led_n];
			if let Some(state) = state {
				match state {
					LEDState::CycleColors => {
						let mut rgb = [c.r, c.g, c.b];
						for i in 0..3 {
							if rgb[i] > 0 {
								if rgb[i] == 254 {
									rgb[i] = 255 // Reach top so we can come down in odds
								} else if rgb[i] == 1 {
									rgb[i] = 0; // Finish this color's cycle
									rgb[(i + 1) % 3] = 2 // Start next color's cycle
								} else if rgb[i] % 2 == 0 {
									// goes up on even, goes down on odd
									rgb[i] += 2
								} else {
									rgb[i] -= 2
								}
							}
						}
						c.r = rgb[0];
						c.g = rgb[1];
						c.b = rgb[2];

						if (c.r == 0 && c.g == 0 && c.b == 0) {
							c.r = 2;
						} // Initialize a color if empty
						 // {c.r=2;c.g=6} // We can even initialize 2 colors at the same time
					}
				}
			}
		}

		// Update LEDs
		for (c, _) in self.led_state {
			self.color(&c);
		}
	}
}

use thread_priority::*;

/// Since leds have to be precisely timed, down to nanoseconds
/// I'll write a thread here without any tokio stuff so we have better control over the runtime
/// Plus tokio will probably be much much slower than we need
pub fn handle_leds(shutdown_rx: watch::Receiver<()>, mut leds: mpsc::Receiver<LEDSyncType>) {
	let pin = Gpio::new().unwrap().get(27).unwrap().into_output();
	let mut array = LedArray {
		pin,
		led_state: Default::default(),
		clock_cycles_per_micro: 0,
	};
	array.led_state[0] = (RGB::default(), Some(LEDState::CycleColors));

	match set_current_thread_priority(ThreadPriority::Max) {
		Ok(_) => {
			info!("Set thread priority to max.")
		}
		Err(err) => {
			info!("Err setting thread priority {err}.")
		}
	}

	let mut n = 30;

	// let corase_time_nanos_10kticks = coarsetime::Duration::from_ticks(10_000).as_nanos();
	// info!("10_000 corasetime_tick_nanos {}ns", corase_time_nanos_10kticks);

	// let start = Instant::now();
	// for _ in 0..1000 {
	// 	let _a = coarsetime::Instant::now() ;
	// }
	// let end = Instant::now();
	// info!("1000 coarsetime::Instant::now() took avg {:?}", (end - start) / 1000);

	// let duration_100nanos = coarsetime::Duration::from_ticks(10_000 * 100 / corase_time_nanos_10kticks);
	// let start = Instant::now();
	// for _ in 0..100 {
	// 	let start_ = coarsetime::Instant::now();
	// 	while coarsetime::Instant::now() - start_ < duration_100nanos {}
	// }
	// let end = Instant::now();
	// info!("100  100ns waits took avg {:?}", (end - start) / 100);

	let start = Instant::now();
	for _ in 0..1000 {
		let _a = Instant::now();
	}
	let end = Instant::now();
	info!("1000 Instant::now() took avg {:?}", (end - start) / 1000);

	let duration_high = Duration::from_nanos(100);
	let start = Instant::now();
	for _ in 0..100 {
		let start_ = Instant::now();
		while Instant::now() - start_ < duration_high {}
	}
	let end = Instant::now();
	info!("100  100ns waits with Instant took avg {:?}", (end - start) / 100);

	// let duration_high = Duration::from_nanos(100);
	array.recalculate_cycles_per_micro();
	info!("cycles_per_micro {}", array.clock_cycles_per_micro);
	let start = Instant::now();
	for _ in 0..1000 {
		array.delay_nanos(100);
	}
	let end = Instant::now();
	info!("1000  100ns waits with delay_nanos took avg {:?}", (end - start) / 1000);

	let start = Instant::now();
	for _ in 0..100 {
		array.bit(true);
	}
	let end = Instant::now();
	info!("100 bits took avg {:?}", (end - start) / 100);

	let start = Instant::now();
	array.reset();
	let end = Instant::now();
	info!("Reset took {:?}", end - start);

	// While we haven't been asked to shut down
	while shutdown_rx.has_changed().is_ok_and(|v| !v) {
		// Update an LED if we were asked to do so
		if let Ok((rgb, state, i)) = leds.try_recv() {
			array.led_state[(i % (array.led_state.len() as u8)) as usize] = (rgb, state);
		}
		// Update all LEDs
		let start = Instant::now();
		array.update();
		let end = Instant::now();
		if n > 0 {
			info!("Update took {:?}", end - start);
			n -= 1;
		}

		// Wait to do next update
		sleep(Duration::from_millis(3000));
	}
	info!("leds shut down.");
}
