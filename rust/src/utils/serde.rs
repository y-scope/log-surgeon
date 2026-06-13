use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::de::Error;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde::ser::SerializeTuple;
use std::marker::PhantomData;

#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct SerdeArray<T>(pub T);

#[derive(Debug, Clone)]
struct ArrayVisitor<T, const N: usize>(PhantomData<[T; N]>);

impl<T, const N: usize> Serialize for SerdeArray<[T; N]>
where
	T: Serialize,
{
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let mut tup: S::SerializeTuple = serializer.serialize_tuple(N)?;
		for element in self.0.iter() {
			tup.serialize_element(element)?;
		}
		tup.end()
	}
}

impl<'de, T, const N: usize> Deserialize<'de> for SerdeArray<[T; N]>
where
	T: Deserialize<'de> + std::fmt::Debug,
{
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let array: [T; N] = deserializer.deserialize_tuple(N, ArrayVisitor::<T, N>(PhantomData))?;
		Ok(SerdeArray(array))
	}
}

impl<'de, T, const N: usize> Visitor<'de> for ArrayVisitor<T, N>
where
	T: Deserialize<'de> + std::fmt::Debug,
{
	type Value = [T; N];

	fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_fmt(format_args!("an array of length {N}"))
	}

	#[inline]
	fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
	where
		A: SeqAccess<'de>,
	{
		let mut elements: Vec<T> = Vec::with_capacity(N);
		for _ in 0..N {
			if let Some(e) = seq.next_element()? {
				elements.push(e);
			} else {
				return Err(Error::invalid_length(N, &self));
			}
		}
		Ok(<[T; N]>::try_from(elements).unwrap())
	}
}

impl<T, const N: usize> std::ops::Deref for SerdeArray<[T; N]> {
	type Target = [T; N];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T, const N: usize> std::ops::DerefMut for SerdeArray<[T; N]> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}
