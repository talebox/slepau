use common::utils::LockedAtomic;
use serde::Serialize;

use super::Media;

/**
 * ChunkView is meant for specific Chunk Data
 * It turns an Rc<Media> to an a specific View of it.
 * This will be customizable based on what the UI needs.
 */
// #[derive(Serialize, Debug, Default)]
// pub struct MediaView {
// 	pub id: MediaId,
// 	pub cache:
// 	pub created: Option<u64>,
// }

// #[derive(PartialEq, Eq, Clone, Copy)]
// pub enum ViewType {
// 	Edit,
// 	Notes,
// 	Well,
// 	Graph,
// }
// impl From<LockedAtomic<Media>> for MediaView {
// 	fn from(value: LockedAtomic<Media>) -> Self {
//       Self::from(&value)
//   }
// }
// impl From<&LockedAtomic<Media>> for MediaView {
// 	fn from(value: &LockedAtomic<Media>) -> Self {
//     let mut value = value.write().unwrap();

//   }
// }

impl From<LockedAtomic<Media>> for Media {
	fn from(rc: LockedAtomic<Media>) -> Self {
		rc.read().unwrap().clone()
	}
}

/**
 * ChunkId is meant for Views
 * It turns an Rc<Media> to an Id String
 */
#[derive(Serialize)]
pub struct MediaId(super::MediaId);
impl From<LockedAtomic<Media>> for MediaId {
	fn from(rc: LockedAtomic<Media>) -> Self {
		Self::from(&rc)
	}
}
impl From<&LockedAtomic<Media>> for MediaId {
	fn from(rc: &LockedAtomic<Media>) -> Self {
		Self(rc.read().unwrap().id.clone())
	}
}

pub enum SortType {
	Created,
}
pub struct MediaVec(pub Vec<LockedAtomic<Media>>);
impl MediaVec {
	pub fn sort(&mut self, t: SortType) {
		self.0.sort_by_cached_key(|v| {
			-(match &t {
				SortType::Created => v.read().unwrap().created,
			} as i64)
		})
	}
}

impl From<Vec<LockedAtomic<Media>>> for MediaVec {
	fn from(v: Vec<LockedAtomic<Media>>) -> Self {
		Self(v)
	}
}
/// Allows turning MediaVec into anything that
impl<T: From<LockedAtomic<Media>>> From<MediaVec> for Vec<T> {
	fn from(val: MediaVec) -> Self {
		val.0.into_iter().map(|v| v.into()).collect()
	}
}
