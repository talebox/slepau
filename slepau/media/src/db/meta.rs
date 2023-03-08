use common::{utils::{get_hash}, proquint::Proquint};
use serde::{Serialize, Deserialize};


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
	pub fn from_img(value: &Vec<u8>) -> Option<Self> {
		let reader = exif::Reader::new();
		let mut b = std::io::BufReader::new(std::io::Cursor::new(value));
		reader
			.read_from_container(&mut b)
			.map(|v| Self(base64::engine::general_purpose::STANDARD_NO_PAD.encode(v.buf().to_owned())))
			.ok()
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
