use std::ffi::c_char;
use std::marker::PhantomData;

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct CUtf8<'lifetime> {
	pointer: *const c_char,
	length: usize,
	_lifetime: PhantomData<&'lifetime [c_char]>,
}

/// Rust is annoying about Send/Sync for pointers, even when it technically **is** safe.
unsafe impl Send for CUtf8<'_> {}
unsafe impl Sync for CUtf8<'_> {}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct SpookyCArray<T> {
	pub pointer: *const T,
	pub length: usize,
}

/// Rust is annoying about Send/Sync for pointers, even when it technically **is** safe.
unsafe impl<T> Send for SpookyCArray<T> {}
unsafe impl<T> Sync for SpookyCArray<T> {}

impl<'lifetime> CUtf8<'lifetime> {
	pub const NULL: Self = Self {
		pointer: std::ptr::null(),
		length: 0,
		_lifetime: PhantomData,
	};

	pub fn new(s: &'lifetime str) -> Self {
		Self {
			pointer: s.as_bytes().as_ptr().cast::<c_char>(),
			length: s.as_bytes().len(),
			_lifetime: PhantomData,
		}
	}

	pub fn as_str(&self) -> &str {
		// TODO unsafe doc comment
		unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.pointer.cast::<u8>(), self.length)) }
	}
}

impl std::ops::Deref for CUtf8<'_> {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		self.as_str()
	}
}

impl<T> SpookyCArray<T> {
	pub const NULL: Self = Self {
		pointer: std::ptr::null(),
		length: 0,
	};
}

impl SpookyCArray<c_char> {
	pub fn from_str(s: &str) -> Self {
		Self {
			pointer: s.as_bytes().as_ptr().cast::<c_char>(),
			length: s.as_bytes().len(),
		}
	}
}
