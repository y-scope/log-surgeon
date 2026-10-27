/// Crate-defined [`TryInto`] to get around Rust's forsaken orphan rules.
pub trait LocalTryInto<T> {
	type Error;

	fn try_into(self) -> Result<T, Self::Error>;
}

impl<T, U> LocalTryInto<U> for T
where
	U: TryFrom<T>,
{
	type Error = U::Error;

	fn try_into(self) -> Result<U, Self::Error> {
		U::try_from(self)
	}
}
