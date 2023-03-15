use std::{io, path::PathBuf};

use common::{proquint::Proquint, utils::get_hash};
use serde::{Deserialize, Serialize};

use base64::Engine as _;
// type my_engine = base64::engine::general_purpose;
#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Exif(String);
impl Exif {
	pub fn to_exif(&self) -> exif::Exif {
		let r = exif::Reader::new();
		r.read_raw(
			base64::engine::general_purpose::STANDARD_NO_PAD
				.decode(self.0.clone())
				.unwrap(),
		)
		.unwrap()
	}
	pub fn from_reader<R: io::BufRead + io::Seek>(value: &mut R) -> Option<Self> {
		let reader = exif::Reader::new();
		reader
			.read_from_container(value)
			.map(|v| Self(base64::engine::general_purpose::STANDARD_NO_PAD.encode(v.buf().to_owned())))
			.ok()
	}
	pub fn from_img(value: &Vec<u8>) -> Option<Self> {
		let mut b = std::io::BufReader::new(std::io::Cursor::new(value));
		Self::from_reader(&mut b)
	}
	pub fn from_path(value: &PathBuf) -> Option<Self> {
		let mut b = std::io::BufReader::new(std::fs::File::open(value).ok()?);
		Self::from_reader(&mut b)
	}
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct FileMeta {
	hash: Proquint<u64>,
	size: usize,
	/// Mime type
	#[serde(rename = "type")]
	pub _type: String,

	#[serde(skip)]
	pub exif: Option<Exif>,
}

impl From<&Vec<u8>> for FileMeta {
	fn from(value: &Vec<u8>) -> Self {
		let _type = infer::get(value);
		let mime_type = _type.map(|v| v.mime_type()).unwrap_or_default();
		let extra = Exif::from_img(value);

		Self {
			hash: get_hash(value).into(),
			size: value.len(),
			_type: mime_type.into(),
			exif: extra,
		}
	}
}

impl FileMeta {
	pub fn from_path(path: &PathBuf) -> Self {
		let _type = infer::get_from_path(path).ok().flatten();
		let mime_type = _type.map(|v| v.mime_type()).unwrap_or_default();
		let extra = Exif::from_path(path);

		Self {
			hash: get_hash(path).into(),
			size: std::fs::metadata(path).map(|v| v.len() as usize).unwrap_or_default(),
			_type: mime_type.into(),
			exif: extra,
		}
	}
}
