/**
 * So basically the whole architecture is divided between backend events
 * and a frontend store.
 *
 * In the backend all we need to manage are a list of (timestamp,event,uniques)
 * which will be the source of truth for the entire store inventory and
 * logistics system.
 *
 * In the frontend we deal with Products and Listings whcih define
 * how the UI groups and sells different items, active listing statuses,
 * variants, categories, prices etc...
 *
 * Store events are optionally complemented by a Set of Items with Uniques containing
 * serial numbers, this means actual products scanned which contain extra data.
 *
 * Each store event type can have optional **unique ids/per event type**
 * that indicate whether items were part of a certain order/shipment/package.
 * These event ids can then have additional data associated with it. Ex an Order
 * will have the name of the customer and their shipping address.
 */
mod defs;

use std::collections::HashMap;

use common::utils::get_secs;
use defs::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
enum StoreError {
	NotEnoughInStock(String),
}

type Stock = HashMap<u32, u32>;
#[derive(Serialize, Deserialize, Debug, Clone)]
struct Store {
	creation_time: u64,
	events: Events,
	items: ItemSet,

	/// The current inventory available
	stock: Stock,
}
impl Default for Store {
	fn default() -> Self {
		Self {
			// When was this store first created
			creation_time: get_secs(),

			events: Default::default(),
			items: Default::default(),
			stock: Default::default(),
		}
	}
}
impl Store {
	pub fn push(&mut self, event: Event) -> Result<(), StoreError> {
		// Perform business logic to make sure the event is valid.

		// Then take stock and push it to the list of events
		self.stock = Self::take_stock_of_event(self.stock.clone(), &event)?;
		self.events.push(event);
		Ok(())
	}

	fn take_stock(&mut self) -> Result<(), StoreError> {
		let mut new_stock = Default::default();
		for event in self.events.iter() {
			new_stock = Self::take_stock_of_event(new_stock, &event)?;
		}
		self.stock = new_stock;
		Ok(())
	}

	fn take_stock_of_event(mut stock: Stock, event: &Event) -> Result<Stock, StoreError> {
		for item in event.items.iter() {
			let item_count = if item.sc.is_count() {
				item.sc.payload()
			} else {
				1
			};
			match event.ty {
				EventType::Stocked | EventType::Cancel { order_id: _ } => {
					stock
						.entry(item.id)
						.and_modify(|v| *v += item_count)
						.or_insert(item_count);
				}
				EventType::Lost | EventType::Order { order_id: _ } => {
					if let Some(stock_item) = stock.get_mut(&item.id) {
						if *stock_item < item_count {
							return Err(StoreError::NotEnoughInStock(format!(
								"Item {:?} only has {:?} in stock. {:?} attemting to remove {:?}.",
								item.id, *stock_item, event.ty, item_count
							)));
						}
						*stock_item -= item_count;
					} else {
						return Err(StoreError::NotEnoughInStock(format!(
							"Item {:?} has never been stocked. {:?} attemting to remove {:?}.",
							item.id, event.ty, item_count
						)));
					}
				}
				_ => {}
			};
		}
		Ok(stock)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn business_logic() {
		let mut store = Store::default();
		assert!(store
			.push(
				// A shipment came in with 4 of item 55
				(EventType::Shipment { shipment_id: 1 }, [(55, 4)]).into(),
			)
			.is_ok());
		assert_eq!(store.stock.get(&55), None);

		assert!(store
			.push(
				// We stocked 4 of item 55
				(EventType::Stocked, [(55, 4)]).into(),
			)
			.is_ok());

		assert_eq!(store.stock.get(&55), Some(&4));

		assert!(store
			.push(
				// Someone ordered 1 of item 55
				(EventType::Order { order_id: 2 }, [(55, 1)]).into(),
			)
			.is_ok());

		assert_eq!(store.stock.get(&55), Some(&3));

		assert!(store
			.push(
				// Lost 2 of item 55
				(EventType::Lost, [(55, 2)]).into(),
			)
			.is_ok());

		assert_eq!(store.stock.get(&55), Some(&1));

		assert!(store
			.push(
				// Fulfilled 1 item 55 for order 2
				(
					EventType::Fullfilled {
						order_id: 2,
						package_id: 1,
					},
					[(55, 2)],
				)
					.into(),
			)
			.is_ok());
		assert!(store
			.push(
				// Someone ordered 1 of item 55 on order 3
				(EventType::Order { order_id: 3 }, [(55, 1)]).into(),
			)
			.is_ok());
		assert_eq!(store.stock.get(&55), Some(&0));
		assert!(store
			.push(
				// Someone cancelled 1 of item 55 on order 3
				(EventType::Cancel { order_id: 3 }, [(55, 1)]).into(),
			)
			.is_ok());
		assert_eq!(store.stock.get(&55), Some(&1));

		assert!(store
			.push(
				// Someone tried ordering 2 of item 55 on order 10
				(EventType::Order { order_id: 10 }, [(55, 2)]).into(),
			)
			.is_err());
		assert_eq!(store.stock.get(&55), Some(&1));
	}
}
