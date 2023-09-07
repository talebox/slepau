use proquint::{QuintError, Quintable};
use rand::{distributions::Standard, prelude::Distribution};
use serde::{de::Visitor, Deserialize, Serialize};

/// Gets serialized/deserialized to/from a proquint -> `lusab_lomad`
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct Proquint<T>(T);
impl<T : Copy> Proquint<T> {
	pub fn inner(&self) -> T {
		return self.0
	}
}
impl<T: Quintable> Serialize for Proquint<T> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serializer.serialize_str(&self.0.to_quint())
	}
}
impl<T: Quintable> Proquint<T> {
	pub fn from_quint(v: &str) -> Result<Self, QuintError> {
		Ok(Self(T::from_quint(v)?))
	}
	pub fn to_quint(&self) -> String {
		self.0.to_quint()
	}
}
impl<T: Quintable> From<T> for Proquint<T> {
	fn from(value: T) -> Self {
		Self(value)
	}
}
impl<T> Default for Proquint<T>
where
	Standard: Distribution<T>,
{
	fn default() -> Self {
		Self(rand::random())
	}
}

use std::{fmt::Display, marker::PhantomData};
#[derive(Default)]
struct ProquintVistor<T> {
	phantom: PhantomData<T>,
}

impl<'de, T: Quintable> Visitor<'de> for ProquintVistor<T> {
	type Value = Proquint<T>;

	fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
		formatter.write_str("a proquint, ex 'lubab_lusan'.")
	}

	fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
	where
		E: serde::de::Error,
	{
		T::from_quint(value)
			.map(|v| v.into())
			.map_err(|_| E::custom("parsing proquint failed."))
	}
}
impl<'de, T: Quintable + Default> Deserialize<'de> for Proquint<T> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		deserializer.deserialize_str(ProquintVistor::default())
	}
}

impl<T: Quintable> Display for Proquint<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(self.to_quint().as_str())
	}
}
