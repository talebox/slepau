use std::{collections::HashMap, error::Error, fmt::Display};

use common::utils::{get_hash, DbError};
use proquint::Quintable;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::MediaId;

impl<'de> Deserialize<'de> for Max {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
		D::Error: serde::de::Error,
	{
		let s = String::deserialize(deserializer)?;
		let split_x = s.split("x").collect::<Vec<_>>();

		if split_x.len() == 2 {
			Ok(Self::Absolute(split_x[0].parse().ok(), split_x[1].parse().ok()))
		} else if s.ends_with("_2") {
			Ok(Self::Area(
				s.replace("_2", "")
					.parse::<usize>()
					.map(|v| v * v)
					.map_err(|_| serde::de::Error::custom("Area invalid"))?,
			))
		} else {
			let u = s.parse().map_err(|_| serde::de::Error::custom("Max invalid"))?;
			Ok(Self::Absolute(Some(u), Some(u)))
		}
	}
}
impl Serialize for Max {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		match self {
			Max::Absolute(x, y) => format!(
				"{}x{}",
				x.map(|v| v.to_string()).unwrap_or_default(),
				y.map(|v| v.to_string()).unwrap_or_default()
			)
			.serialize(serializer),
			Max::Area(a) => format!("{}_2", (a.to_owned() as f32).sqrt() as usize).serialize(serializer),
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Max {
	Absolute(Option<usize>, Option<usize>),
	Area(usize),
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, default)]
pub struct Version {
	#[serde(rename = "type", skip_serializing_if = "Option::is_none")]
	pub _type: Option<String>,
	/// Defines a max ^2 squared size for the image.
	///
	/// Such a way that if max = 100, that means image will be capped at 100*100 px.
	/// That means image can be 10 * 1000, or 1 * 10000, this cap is only pixel-wize.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub max: Option<Max>,
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
				.map(|(k, v)| format!("{k}={}", v.as_str().map(|v| v.to_string()).unwrap_or(v.to_string())))
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
				(key.to_string(), json!(value))
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

#[cfg(test)]
mod test {
	use super::VersionString;

	#[test]
	fn version_string() {
		assert_eq!(
			"type=img/test",
			VersionString::from("type=img/test").0,
			"Should parse strings without quotes correctly."
		);
		assert_eq!(
			"max=100x100&type=img/test",
			VersionString::from("max=100&type=img/test").0,
			"Should only allow Version key and should reorder alphabetically."
		);
		assert_eq!(
			"max=x100&type=img/test",
			VersionString::from("max=x100&type=img/test").0,
			"Should only allow Version key and should reorder alphabetically."
		);
		assert_eq!(
			"max=100x&type=img/test",
			VersionString::from("max=100x&type=img/test").0,
			"Should only allow Version key and should reorder alphabetically."
		);
		assert_eq!(
			"max=100_2&type=img/test",
			VersionString::from("max=100_2&type=img/test").0,
			"Should only allow Version key and should reorder alphabetically."
		);
	}
}
