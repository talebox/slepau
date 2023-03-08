use std::{collections::HashMap, fmt::Display};

use common::utils::{get_hash, DbError};
use proquint::Quintable;
use serde::{Serialize, Deserialize};
use serde_json::{Value, json};

use super::MediaId;

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, default)]
pub struct Version {
	#[serde(rename = "type", skip_serializing_if = "Option::is_none")]
	pub _type: Option<String>,
	// #[serde(skip_serializing_if = "Option::is_none")]
	// xm: Option<usize>,
	// #[serde(skip_serializing_if = "Option::is_none")]
	// ym: Option<usize>,
	
	/// Defines a max ^2 squared size for the image.
	///
	/// Such a way that if max = 100, that means image will be capped at 100*100 px.
	/// That means image can be 10 * 1000, or 1 * 10000, this cap is only pixel-wize.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub size: Option<usize>,
}
/// Encodes a version as a string.
///
/// The encoding should be normalized so if two versions have the same data, they are the same.
#[derive(Default, Serialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VersionString(pub String);
// pub type VersionString = String;

/// A version reference has everything needed to figure out a path to the data.
///
/// It's the combination of Media + Version
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VersionReference {
	pub id: MediaId,
	pub version: VersionString,
}
impl From<(MediaId, VersionString)> for VersionReference {
	fn from((id, version): (MediaId, VersionString)) -> Self {
		Self { id, version }
	}
}
impl VersionReference {
	pub fn to_filename(&self) -> String {
		get_hash(self).to_quint()
	}
}

impl From<&Version> for VersionString {
	fn from(value: &Version) -> Self {
		Self(
			serde_json::to_value(value)
				.unwrap()
				.as_object()
				.unwrap()
				.iter()
				.map(|(k, v)| format!("{k}={v}"))
				.collect::<Vec<_>>()
				.join("&")
				.to_string(),
		)
	}
}
impl From<&VersionString> for Version {
	fn from(value: &VersionString) -> Self {
		value.to_version().unwrap()
	}
}

impl From<&str> for VersionString {
	fn from(value: &str) -> Self {
		Self::new(value).unwrap()
	}
}
impl VersionString {
	pub fn new(value: &str) -> Result<Self, DbError> {
		// &str -> VersionString
		let s = Self(value.into());
		// VersionString -> Version
		let s = s.to_version()?;
		// Version -> VersionString
		let s = Self::from(&s);
		Ok(s)
	}
	pub fn to_version(&self) -> Result<Version, DbError> {
		if self.0.is_empty() {
			return Ok(Default::default());
		}

		let value = self.0.split("&").map(|v| v.split("=").collect::<Vec<_>>());

		if value.clone().any(|v| v.len() != 2) {
			return Err("All records (separated by '&') to have exactly 1 key and 1 value separated by an '='.".into());
		}
		let value = value
			.map(|v| {
				let key = v[0];
				let value = v[1];
				(
					key.to_string(),
					serde_json::from_str::<Value>(value)
						.unwrap_or_else(|_| serde_json::from_str::<Value>(&format!("\"{}\"", value)).unwrap()),
				)
			})
			.collect::<HashMap<_, _>>();

		Ok(
			serde_json::from_value(json!(value))
				.map_err(|err| DbError::from(format!("Serde parsing Error: {err}").as_str()))?,
		)
	}
}

impl Display for VersionString {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}
impl<'de> Deserialize<'de> for VersionString {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Ok(Self::from(String::deserialize(deserializer)?.as_str()))
	}
}