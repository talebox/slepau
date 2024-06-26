use std::{
	collections::{BTreeMap, HashMap},
	time::SystemTime,
};

use common::{proquint::Proquint, samn::decode_binary_base64};
use samn_common::node::{Limb, LimbId, LimbType, NodeId, NodeInfo};
use serde::{Deserialize, Serialize};
use sonnerie::Wildcard;

use crate::db::{NodeUiData, DB};

#[derive(Serialize, Deserialize, Debug)]
pub struct LimbPreview {
	pub id: LimbId,
	pub data: LimbType,
	/// Last message received, in epoch seconds
	pub last: u64,
}

#[derive(Serialize, Default, Deserialize, Debug)]
pub struct NodePreview {
	pub ui: NodeUiData,
	pub id: Proquint<NodeId>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub info: Option<NodeInfo>,
	pub limbs: HashMap<LimbId, LimbPreview>,
	/// Last message received, in epoch seconds
	pub last: u64,
	/// Node uptime, in seconds
	pub uptime: u32,
}

pub fn node_previews(db: &DB, key: String) -> HashMap<Proquint<NodeId>, NodePreview> {
	let uptime = |node_id| db.heartbeats.get(&node_id).map(|(_, last, uptime,_)| (*last, *uptime)).unwrap_or((0,0));
	common::sonnerie::db().get_filter(&Wildcard::new(&key)).into_iter().fold(
		HashMap::new(),
		|mut acc, r | {
			let key_split = r.key().split("_").collect::<Vec<_>>();
			// let id_node: String = .into();
			let id_node = Proquint::<u32>::from_quint(&key_split[0..2].join("_")).unwrap();
			let id_limb = key_split.get(2).map(|v| LimbId::from_str_radix(v, 16).unwrap());
			let time = r.timestamp_nanos();
			let (last,uptime) = uptime(id_node.inner());

			let node = acc.entry(id_node);
			if let Some(id_limb) = id_limb {
				// let id_limb = Proquint::<u16>::from_quint(&id_limb).unwrap();
				// Deserialize Limb
				let mut bytes = r.get::<String>(0).into_bytes();
				let limb: Limb = decode_binary_base64(&mut bytes);

				node
					.and_modify(|node| {
						node
							.limbs
							.entry(id_limb)
							.and_modify(|limb_| {
								if time > limb_.last {
									limb_.data = limb.1.clone();
									limb_.last = time;
								}
							})
							.or_insert(LimbPreview {
								id: id_limb,
								data: limb.1.clone(),
								last: time,
							});
					})
					.or_insert(NodePreview {
						id: id_node,
						limbs: HashMap::from([(
							id_limb,
							LimbPreview {
								id: id_limb,
								data: limb.1.clone(),
								last: time,
							},
						)]),
						last,
						uptime,
						info: Default::default(),
						ui: db.node_ui_data.get(&id_node.inner()).cloned().unwrap_or_default()
					});
			} else {
				
				// Deserialize Info
				let mut bytes = r.get::<String>(0).into_bytes();
				let info: NodeInfo = decode_binary_base64(&mut bytes);
				node
					.and_modify(|node| {
						if time > node.last {
							node.info = Some(info.clone());
							node.last = time;
						}
					})
					.or_insert(NodePreview {
						id: id_node,
						info: Some(info),
						last,
						uptime,
						limbs: Default::default(),
						ui: db.node_ui_data.get(&id_node.inner()).cloned().unwrap_or_default()
					});
			}
			return acc;
		},
	)
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct LimbQuery {
	pub node_id: Proquint<NodeId>,
	pub limb_id: LimbId,

	pub period: usize, // in sec
	pub limit: usize,  // in # of periods
}
impl Default for LimbQuery {
	fn default() -> Self {
		Self {
			node_id: Default::default(),
			limb_id: Default::default(),

			period: 60 * 10, // 10 min
			limit: 24,       // 24 periods (10 min, 4 hrs)
		}
	}
}

pub fn limb_history(query: LimbQuery) -> BTreeMap<u64, LimbType> {
	let now = SystemTime::now()
		.duration_since(SystemTime::UNIX_EPOCH)
		.unwrap()
		.as_secs();

	common::sonnerie::db()
		.get(&format!("{}_{:02X}", query.node_id, query.limb_id))
		.into_iter()
		.fold(BTreeMap::new(), |mut acc, r| {
			let time = r.timestamp_nanos() / 1_000_000_000; // Seconds
			let time_diff = now - time;
			if time_diff as usize >= query.limit * query.period {
				return acc;
			}

			let mut bytes = r.get::<String>(0).into_bytes();
			let limb: Limb = decode_binary_base64(&mut bytes);
			let limb_type = limb.1;
			let time_entry = time_diff / query.period as u64;
			acc
				.entry(time_entry)
				.and_modify(|v| *v = v.clone() + limb_type.clone())
				.or_insert(limb_type);

			acc
		})
}
