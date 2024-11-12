//! The definition for samn's database values

use chrono::Utc;
use proquint::Quintable;
use samn_common::node::{Limb, NodeInfo};
use serde::{Deserialize, Serialize};
use sonnerie::record;
use base64::prelude::*;

use crate::sonnerie::{commit, transaction};

pub fn encode_binary_base64<T:Serialize>(v: &T) -> String {
	BASE64_STANDARD.encode(postcard::to_vec::<T,32>(v).unwrap())
}
pub fn decode_binary_base64<'a,T:Deserialize<'a>>(v: &'a mut [u8]) -> T {
	let input = v.to_owned();
	BASE64_STANDARD.decode_slice(input, v).unwrap();
	postcard::from_bytes(v).unwrap()
} 

/// Logs limbs as \<id\>_01
pub fn log_limbs(id: u32, limbs: &[Limb]) {
	let mut t = transaction();
	for limb in limbs {
		let key = format!("{}_{:02X}", id.to_quint(), limb.0);
		let str = encode_binary_base64(limb);
		t.add_record(&key, Utc::now().naive_utc(), record(str))
			.unwrap();
	}
	commit(t);
}
pub fn log_info(id: u32, info: &NodeInfo) {
    let mut t = transaction();
	let str = encode_binary_base64(info);
    t.add_record(&id.to_quint(), Utc::now().naive_utc(), record(str))
			.unwrap();
    commit(t);
}