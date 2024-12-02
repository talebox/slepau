use samn_common::node::{Command, NodeId};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime, PrimitiveDateTime, Weekday};

use super::DB;


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventTime {
	/// 0-59
	#[serde(skip_serializing_if = "Option::is_none")]
	minute: Option<u8>,
	/// 0-23
	#[serde(skip_serializing_if = "Option::is_none")]
	hour: Option<u8>,
	/// 1-31
	#[serde(skip_serializing_if = "Option::is_none")]
	month_day: Option<u8>,
	/// 1-12
	#[serde(skip_serializing_if = "Option::is_none")]
	month: Option<u8>,
	/// 0-6  Sunday=0 or 7
	#[serde(skip_serializing_if = "Option::is_none")]
	week_day: Option<u8>,
}


impl EventTime {
	pub fn new(
		minute: Option<u8>,
		hour: Option<u8>,
		month_day: Option<u8>,
		month: Option<u8>,
		week_day: Option<u8>,
	) -> Self {
		Self {
			minute,
			hour,
			month_day,
			month,
			week_day,
		}
	}
	// Checks if the given time matches the EventTime schedule
	pub fn matches(&self, time: &OffsetDateTime) -> bool {
		if let Some(minute) = self.minute {
			if time.minute() != minute {
				return false;
			}
		}
		if let Some(hour) = self.hour {
			if time.hour() != hour {
				return false;
			}
		}
		if let Some(month_day) = self.month_day {
			if time.day() != month_day {
				return false;
			}
		}
		if let Some(month) = self.month {
			if time.month() as u8 != month {
				return false;
			}
		}
		if let Some(week_day) = self.week_day {
			// In `time` crate, Sunday is 0
			let wd = time.weekday().number_days_from_sunday();
			if wd != week_day % 7 {
				return false;
			}
		}
		true
	}

	/// Gives seconds since last event
	/// Only checks last 30 minutes
	pub fn since(&self, time: &OffsetDateTime) -> Option<u32> {
		let mut t = *time;

		for _ in 0..30 {
			if self.matches(&t) {
				let duration = *time - t;
				return Some(duration.whole_seconds() as u32);
			}
			// Decrement time by one minute
			t = match t.checked_sub(Duration::minutes(1)) {
				Some(new_time) => new_time,
				None => break, // If time underflows, break the loop
			};
		}
		None
	}
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
	pub id: NodeId,
	pub time: EventTime,
	pub command: Command,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Schedule {
	pub events: Vec<Event>,
}

// impl DB {
//   /// Maybe queue update already calls this, shouldn't be called directly
//   pub fn maybe_queue_schedule_update(&mut self, id_node_db: u32) -> bool {
//     self.
//   }
// }

#[cfg(test)]
mod tests {
	use common::proquint::Proquint;
	use samn_common::node::{Actuator, Limb, LimbType};
	use time::{macros::{datetime, offset}, util::local_offset::Soundness};

	use super::*;

	#[test]
	fn test_schedule() {
		let schedule = make_test_schedule();
		assert!(event_matches(&schedule, &datetime!(2020-01-01 0:00 UTC)));
		assert!(event_matches(&schedule, &datetime!(2020-01-01 9:00 UTC)));
		assert!(event_matches(&schedule, &datetime!(2020-01-01 20:00 UTC)));
		assert!(event_matches(&schedule, &datetime!(2020-01-01 22:00 UTC)));

		assert!(event_matches(&schedule, &datetime!(2020-01-01 5:00 UTC).to_offset(offset!(-5))));
		assert!(event_matches(&schedule, &datetime!(2020-01-01 14:00 UTC).to_offset(offset!(-5))));
		assert!(event_matches(&schedule, &datetime!(2020-01-01 1:00 UTC).to_offset(offset!(-5))));
		assert!(event_matches(&schedule, &datetime!(2020-01-01 3:00 UTC).to_offset(offset!(-5))));

		assert!(!event_matches(&schedule, &datetime!(2020-01-01 0:01 UTC)));
		assert!(!event_matches(&schedule, &datetime!(2020-01-01 11:01 UTC)));
		assert!(!event_matches(&schedule, &datetime!(2020-01-01 23:00 UTC)));
	}
	#[test]
	fn get_local_time() {
		unsafe {time::util::local_offset::set_soundness(Soundness::Unsound)};
		time::OffsetDateTime::now_local().unwrap();
		// assert!();
	}
	fn event_matches(schedule: &Schedule, now: &OffsetDateTime) -> bool {
		schedule.events.iter().any(|event|event.time.matches(now))
	}
	fn make_test_schedule() -> Schedule {
		Schedule {
			events: vec![
				// Kitchen at 9am
				Event {
					id: Proquint::<samn_common::node::NodeId>::from_quint("hizig_dujig")
						.unwrap()
						.inner(),
					time: EventTime::new(Some(0), Some(9), None, None, None),
					command: Command::SetLimb(Limb(1, LimbType::Actuator(Actuator::Light(true)))),
				},
				// Kitchen at 12am
				Event {
					id: Proquint::<samn_common::node::NodeId>::from_quint("hizig_dujig")
						.unwrap()
						.inner(),
					time: EventTime::new(Some(0), Some(0), None, None, None),
					command: Command::SetLimb(Limb(1, LimbType::Actuator(Actuator::Light(false)))),
				},
				// Living Room at 8pm
				Event {
					id: Proquint::<samn_common::node::NodeId>::from_quint("sonoh_giguk")
						.unwrap()
						.inner(),
					time: EventTime::new(Some(0), Some(20), None, None, None),
					command: Command::SetLimb(Limb(1, LimbType::Actuator(Actuator::Light(true)))),
				},
				// Living Room at 10pm
				Event {
					id: Proquint::<samn_common::node::NodeId>::from_quint("sonoh_giguk")
						.unwrap()
						.inner(),
					time: EventTime::new(Some(0), Some(22), None, None, None),
					command: Command::SetLimb(Limb(1, LimbType::Actuator(Actuator::Light(false)))),
				},
			],
		}
	}
}
