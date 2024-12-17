use std::{collections::HashMap, num::ParseIntError, str::FromStr};

use common::{
	proquint::Proquint,
	utils::{REGEX_ALIAS, REGEX_EVENT},
};
use samn_common::node::{Actuator, Command, Limb, LimbType, NodeId};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use super::DB;


#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
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
impl FromStr for EventTime {
	type Err = ParseIntError;
	fn from_str(mut v: &str) -> Result<Self, Self::Err> {
		v = v.trim();
		let mut is_pm = false;
		let mut hour = 0;
		let mut minute = 0;

		if v.ends_with("pm") {
			is_pm = true;
			v = &v[..v.len() - 2];
		} else if v.ends_with("am") {
			v = &v[..v.len() - 2];
		}

		v = v.trim();

		if v == "noon" {
			hour = 12;
		} else if v == "midnight" {
			hour = 0; // Basically do nothing
		} else if v.contains(":") {
			for (i, v) in v.split(":").filter(|v| v.len() > 0).enumerate() {
				match i {
					0 => {
						hour = v.parse()?;
					}
					1 => {
						minute = v.parse()?;
					}
					_ => {}
				}
			}
		} else {
			hour = v.parse()?;
		}

		if is_pm {
			hour += 12;
		}

		Ok(Self::new_hr_min(hour, minute))
	}
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
			minute: minute.map(|v| v % 60),
			hour: hour.map(|v| v % 24),
			month_day,
			month,
			week_day: week_day.map(|v| v % 7),
		}
	}
	pub fn new_hr_min(hour: u8, minute: u8) -> Self {
		Self {
			minute: Some(minute),
			hour: Some(hour),
			month_day: None,
			month: None,
			week_day: None,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Event {
	pub id: NodeId,
	pub time: EventTime,
	pub command: Command,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
	pub events: Vec<Event>,
}

impl From<&str> for Schedule {
	fn from(value: &str) -> Self {
		let aliases = REGEX_ALIAS
			.captures_iter(value)
			.filter_map(|c| {
				if let (Some(id), Some(name)) = (
					c.get(1)
						.and_then(|id| Proquint::<NodeId>::from_quint(id.as_str()).ok()),
					c.get(2).map(|v| v.as_str().to_string()),
				) {
					Some((name, id))
				} else {
					None
				}
			})
			.collect::<HashMap<_, _>>();

		// WARNING, this HAS to be changed later after an update to sensors for Command SetLimbType
		fn command_from_str(limb_id: Option<u8>, s: &str) -> Option<Command> {
			use self::Actuator::Light;
			use Command::SetLimb;
			use LimbType::Actuator;
			let limb_id = limb_id.unwrap_or(1);
			match s {
				"light_on" => Some(SetLimb(Limb(limb_id, Actuator(Light(true))))),
				"light_off" => Some(SetLimb(Limb(limb_id, Actuator(Light(false))))),
				"light on" => Some(SetLimb(Limb(limb_id, Actuator(Light(true))))),
				"light off" => Some(SetLimb(Limb(limb_id, Actuator(Light(false))))),
				_ => None,
			}
		}

		let events = REGEX_EVENT
			.captures_iter(value)
			.filter_map(|c| {
				if let (Some(time), Some(id), Some(command)) = (
					c.name("time").and_then(|x| x.as_str().parse().ok()),
					c.name("id").and_then(|x| {
						aliases
							.get(x.as_str())
							.cloned()
							.or_else(|| x.as_str().parse().ok())
					}),
					c.name("c").and_then(|x| {
						command_from_str(
							c.name("lid").and_then(|x| x.as_str().parse().ok()),
							x.as_str(),
						)
					}),
				) {
					Some(Event {
						time,
						id: id.inner(),
						command,
					})
				} else {
					None
				}
			})
			.collect::<Vec<_>>();

		Self { events }
	}
}

impl DB {
	pub fn set_schedule(&mut self, schedule_raw: String) {
		self.schedule_raw = schedule_raw;
		self.schedule = self.schedule_raw.as_str().into();
	}
}

#[cfg(test)]
mod tests {
	use common::proquint::Proquint;
	use samn_common::node::{Actuator, Limb, LimbType};
	use time::{
		macros::{datetime, offset},
		util::local_offset::Soundness,
	};

	use super::*;

	#[test]
	fn test_schedule() {
		let schedule = make_test_schedule();
		assert!(event_matches(&schedule, &datetime!(2020-01-01 0:00 UTC)));
		assert!(event_matches(&schedule, &datetime!(2020-01-01 9:00 UTC)));
		assert!(event_matches(&schedule, &datetime!(2020-01-01 20:00 UTC)));
		assert!(event_matches(&schedule, &datetime!(2020-01-01 22:00 UTC)));

		assert!(event_matches(
			&schedule,
			&datetime!(2020-01-01 5:00 UTC).to_offset(offset!(-5))
		));
		assert!(event_matches(
			&schedule,
			&datetime!(2020-01-01 14:00 UTC).to_offset(offset!(-5))
		));
		assert!(event_matches(
			&schedule,
			&datetime!(2020-01-01 1:00 UTC).to_offset(offset!(-5))
		));
		assert!(event_matches(
			&schedule,
			&datetime!(2020-01-01 3:00 UTC).to_offset(offset!(-5))
		));

		assert!(!event_matches(&schedule, &datetime!(2020-01-01 0:01 UTC)));
		assert!(!event_matches(&schedule, &datetime!(2020-01-01 11:01 UTC)));
		assert!(!event_matches(&schedule, &datetime!(2020-01-01 23:00 UTC)));
	}
	#[test]
	fn get_local_time() {
		unsafe { time::util::local_offset::set_soundness(Soundness::Unsound) };
		time::OffsetDateTime::now_local().unwrap();
		// assert!();
	}
	#[test]
	fn test_time_parsing() {
		assert_eq!(EventTime::new_hr_min(12, 20), "12:20".parse().unwrap());
		assert_eq!(EventTime::new_hr_min(12, 20), "0:20pm".parse().unwrap());
		assert_eq!(EventTime::new_hr_min(22, 01), "10:01pm".parse().unwrap());
	}
	#[test]
	fn test_schedule_parser() {
		let mut tschedule = make_test_schedule();

		assert_eq!(
			Schedule::from(
				r#"
				at 9 for hizig_dujig set light on
				at 0 for hizig_dujig set light off
				at 20 for sonoh_giguk set light on
				at 22 for sonoh_giguk set light off
				"#
			),
			tschedule
		);
		assert_eq!(
			Schedule::from(
				r#"
				at 9am for hizig_dujig set light on
				at midnight for hizig_dujig set light off
				at 8pm for sonoh_giguk set light on
				at 10pm for sonoh_giguk set light off
				"#
			),
			tschedule
		);
		assert_eq!(
			Schedule::from(
				r#"
				alias hizig_dujig kitchen_light
				alias sonoh_giguk living_room_light

				at 9am for kitchen_light set light on
				at midnight for kitchen_light set light off

				at 8pm for living_room_light set light on
				at 10pm for living_room_light set light off
				"#
			),
			tschedule
		);

		tschedule.events[0].command = Command::SetLimb(Limb(2, LimbType::Actuator(Actuator::Light(true)) ));
		tschedule.events[0].time.minute = Some(40);
		tschedule.events[0].time.hour = Some(21);
		assert_eq!(
			Schedule::from(
				r#"
				alias hizig_dujig kitchen_light
				alias sonoh_giguk living_room_light

				at 9:40pm for kitchen_light set 2 light on
				at midnight for kitchen_light set light off

				at 8pm for living_room_light set light on
				at 10pm for living_room_light set light off
				"#
			),
			tschedule
		);
	}
	fn event_matches(schedule: &Schedule, now: &OffsetDateTime) -> bool {
		schedule.events.iter().any(|event| event.time.matches(now))
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
