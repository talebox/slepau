use common::{
	proquint::{Proquint, Quintable},
	utils::get_secs,
};
use nonmax::NonMaxU32;
use serde::{Deserialize, Serialize};
use std::{
	borrow::Borrow,
	collections::{HashMap, HashSet},
	hash::{Hash, Hasher},
	ops::Add,
	u32,
};

#[derive(
	Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[repr(transparent)]
pub struct SOrCount(u32);
impl SOrCount {
	const TAG: u32 = 1 << 31;
	const PAYLOAD_MASK: u32 = !(1 << 31);

	/// Create a Serial. Valid for 0..=0x7FFF_FFFF.
	pub fn serial(v: u32) -> Self {
		if v <= Self::PAYLOAD_MASK {
			Self(v)
		} else {
			panic!("{v} bigger than 0x7FFF_FFFF")
		}
	}
	/// Create a Count. Valid for 0..=0x7FFF_FFFF.
	pub fn count(v: u32) -> Self {
		if v <= Self::PAYLOAD_MASK {
			Self(Self::TAG | v)
		} else {
			panic!("{v} bigger than 0x7FFF_FFFF")
		}
	}

	pub const fn is_serial(self) -> bool {
		!self.is_count()
	}
	pub const fn is_count(self) -> bool {
		(self.0 & Self::TAG) != 0
	}
	pub const fn payload(self) -> u32 {
		self.0 & Self::PAYLOAD_MASK
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn sorcount() {
		let serial = SOrCount::serial(4);
		assert!(serial.is_serial());
		assert_eq!(serial.payload(), 4);

		let count = SOrCount::count(4);
		assert!(count.is_count());
		assert_eq!(count.payload(), 4);

		let serial = SOrCount::serial(SOrCount::PAYLOAD_MASK);
		assert!(serial.is_serial());
		assert_eq!(serial.payload(), SOrCount::PAYLOAD_MASK);

		let count = SOrCount::count(SOrCount::PAYLOAD_MASK);
		assert!(count.is_count());
		assert_eq!(count.payload(), SOrCount::PAYLOAD_MASK);
	}
	#[test]
	#[should_panic]
	fn sorcount_invalid() {
		let count = SOrCount::count(SOrCount::PAYLOAD_MASK + 1);
	}
}


/// Defines either a unique item (id+serial) or a nunber of items (id+count)
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ItemKey {
	/// Internal item number
	pub(super) id: u32,
	/// Item serial number / count of items
	pub(super) sc: SOrCount,
}
impl From<(u32, SOrCount)> for ItemKey {
	fn from(value: (u32, SOrCount)) -> Self {
		Self {
			id: value.0,
			sc: value.1,
		}
	}
}
impl From<(u32, u32)> for ItemKey {
	fn from(value: (u32, u32)) -> Self {
		Self {
			id: value.0,
			sc: SOrCount::count(value.1),
		}
	}
}

// impl Quintable for ItemKey {
// 	fn from_quint(_str: &str) -> Result<Self, common::proquint::QuintError> {
// 		if let Ok(id_serial) = u64::from_quint(_str) {
// 			Ok(Self::from((
// 				(id_serial >> 32) as u32,
// 				Some(
// 					NonMaxU32::try_from(id_serial as u32)
// 						.expect("The serial in the quint was u32::MAX? we couldn't convert."),
// 				),
// 			)))
// 		} else {
// 			Ok(Self::from(u32::from_quint(_str)?))
// 		}
// 	}
// 	fn to_quint(&self) -> String {
// 		if let Some(serial) = self.serial {
// 			((self.id as u64) << 32 | serial.get() as u64).to_quint()
// 		} else {
// 			self.id.to_quint()
// 		}
// 	}
// }


/// A unique instance of a product.
///
/// Hashes and equality are based on the Unique property.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Item {
	// Internal item number + item serial number
	unique: ItemKey,
	/// GTIN (Global Trade Item Number)
	gtin: Option<u64>,
	prod_date: Option<u32>,
	variant: Option<u8>,
	batch: Option<String>,
}
impl PartialEq for Item {
	fn eq(&self, other: &Self) -> bool {
		self.unique == other.unique
	}
}
impl Eq for Item {}
impl Hash for Item {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.unique.hash(state)
	}
}
impl Borrow<ItemKey> for Item {
	fn borrow(&self) -> &ItemKey {
		&self.unique
	}
}
pub type ItemSet = HashSet<Item>;


pub struct Shipment {
	id: u32,
	/// Expected arrival timestamp
	expected: u64,
}

pub struct Address {
	// Second line
	street: String,
	apt_suite: Option<String>,
	// Third line
	city: String,
	state: String,
	zip: String,
}

pub struct Order {
	id: u32,
	// First line
	name: String,
	address: Address,
}

pub struct Package {
	id: u32,
	tracking: Option<String>,
	carrier: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum EventType {
	/// A shipment is arriving with more items.
	Shipment { shipment_id: u32 },
	/// Items were stocked, available for purchase
	Stocked,
	/// Items were lost
	Lost,
	/// An order was made
	Order { order_id: u32 },
	/// Items some/all from an order were cancelled
	Cancel { order_id: u32 },
	/// An order has been fulfilled.
	/// A shipment with products has been made to the customer.
	Fullfilled { order_id: u32, package_id: u32 },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
	pub time: u64,
	pub ty: EventType,
	pub items: Vec<ItemKey>,
}
impl From<(EventType, Vec<ItemKey>)> for Event {
	fn from(value: (EventType, Vec<ItemKey>)) -> Self {
		Self {
			time: get_secs(),
			ty: value.0,
			items: value.1,
		}
	}
}
impl<const N: usize> From<(EventType, [(u32, u32); N])> for Event {
	fn from((et, items): (EventType, [(u32, u32); N])) -> Self {
		let items: Vec<ItemKey> = Vec::from(items.map(Into::into));
		Self::from((et, items))
	}
}
pub type Events = Vec<Event>;
