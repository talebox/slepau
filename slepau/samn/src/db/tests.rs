#[test]
fn addresses() {
	use crate::db::{DB, HQADDRESS};
	let mut db = DB::default();

	// New address
	assert_eq!(db.issue_address(16, true), (HQADDRESS + 1));
	// Address 1 above the one before
	assert_eq!(db.issue_address(22, true), (HQADDRESS + 2));
	// Same address as before, nodeid exists
	assert_eq!(db.issue_address(22, false), (HQADDRESS + 2));
	// For cc1101 is 1 below
	assert_eq!(db.issue_address(324, false), (HQADDRESS - 1));
}

// #[test]
// fn message_size() {
//   use samn_common::node::*;
//   let message = Message::Message(MessageData::Response {
//     id: None,
//     response: Response::Heartbeat(64),
//   });
//   let v: heapless::Vec<u8, 32> = postcard::to_vec(&message).unwrap();
//   println!("Hearbeat {} {v:?}", v.len());

//   let message = Message::Message(MessageData::Response {
//     id: None,
//     response: Response::Limbs([Some(Limb(1, LimbType::Actuator(Actuator::Light(true)))), None, None]),
//   });
//   let v: heapless::Vec<u8, 32> = postcard::to_vec(&message).unwrap();
//   println!("Limbs {} {v:?}", v.len());
// }
