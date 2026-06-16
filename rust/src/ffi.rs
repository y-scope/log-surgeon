// Necessary for FFI.
#![allow(clippy::box_collection)]

use std::ffi::c_char;
use std::marker::PhantomData;
use std::str::Utf8Error;

/// Represents a C `T const*` pointer + `size_t` length as a single ABI-stable value;
/// intended for receiving arguments from FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CArray<'lifetime, T> {
	pointer: *const T,
	length: usize,
	_lifetime: PhantomData<&'lifetime [T]>,
}

/// Rust is annoying about Send/Sync for pointers, even when it technically **is** safe.
unsafe impl<T> Send for CArray<'_, T> {}
unsafe impl<T> Sync for CArray<'_, T> {}

impl<T: Eq> Eq for CArray<'_, T> {}

impl<T: Ord> Ord for CArray<'_, T> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.as_slice().cmp(other.as_slice())
	}
}

impl<T: PartialEq> PartialEq for CArray<'_, T> {
	fn eq(&self, other: &Self) -> bool {
		self.as_slice().eq(other.as_slice())
	}
}

impl<T: PartialOrd> PartialOrd for CArray<'_, T> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		self.as_slice().partial_cmp(other.as_slice())
	}
}

pub type CCharArray<'lifetime> = CArray<'lifetime, c_char>;

/// A rust `&str` accessible in FFI;
/// unlike [`CCharArray`], this is already guaranteed to be UTF-8.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct CUtf8<'lifetime> {
	pointer: *const c_char,
	length: usize,
	_lifetime: PhantomData<&'lifetime [c_char]>,
}

/// Rust is annoying about Send/Sync for pointers, even when it technically **is** safe.
unsafe impl Send for CUtf8<'_> {}
unsafe impl Sync for CUtf8<'_> {}

impl Eq for CUtf8<'_> {}

impl Ord for CUtf8<'_> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.as_str().cmp(other.as_str())
	}
}

impl PartialEq for CUtf8<'_> {
	fn eq(&self, other: &Self) -> bool {
		self.cmp(other).is_eq()
	}
}

impl PartialOrd for CUtf8<'_> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

/// Can't use `std::range::Range` because it's not `#[repr(C)]`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct CRange<Idx> {
	pub start: Idx,
	pub end: Idx,
}

/// A pointer-length pair with unchecked/untied lifetime.
#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct UncheckedCArray<T> {
	pub pointer: *const T,
	pub length: usize,
}

/// Rust is annoying about Send/Sync for pointers, even when it technically **is** safe.
unsafe impl<T> Send for UncheckedCArray<T> {}
unsafe impl<T> Sync for UncheckedCArray<T> {}

impl<'lifetime, T> CArray<'lifetime, T> {
	pub const fn null() -> Self {
		Self {
			pointer: std::ptr::null(),
			length: 0,
			_lifetime: PhantomData,
		}
	}

	pub fn from_slice(slice: &'lifetime [T]) -> Self {
		Self {
			pointer: slice.as_ptr(),
			length: slice.len(),
			_lifetime: PhantomData,
		}
	}

	pub fn as_slice(&self) -> &'lifetime [T] {
		unsafe { std::slice::from_raw_parts(self.pointer, self.length) }
	}
}

impl<T> std::ops::Deref for CArray<'_, T> {
	type Target = [T];

	fn deref(&self) -> &Self::Target {
		self.as_slice()
	}
}

impl<'lifetime> CCharArray<'lifetime> {
	pub fn from_utf8(utf8: &'lifetime str) -> Self {
		// `str::len` is indeed the byte length,
		// as opposed to the number of unicode characters/code points,
		// but let's be explicit.
		Self {
			pointer: utf8.as_bytes().as_ptr().cast::<c_char>(),
			length: utf8.as_bytes().len(),
			_lifetime: PhantomData,
		}
	}

	pub fn as_utf8(&self) -> Result<&'lifetime str, Utf8Error> {
		let bytes: &[u8] = unsafe { std::slice::from_raw_parts(self.pointer.cast::<u8>(), self.length) };
		str::from_utf8(bytes)
	}
}

impl<T> UncheckedCArray<T> {
	pub const NULL: Self = Self {
		pointer: std::ptr::null(),
		length: 0,
	};
}

impl UncheckedCArray<c_char> {
	pub fn new(s: &str) -> Self {
		Self {
			pointer: s.as_bytes().as_ptr().cast::<c_char>(),
			length: s.as_bytes().len(),
		}
	}

	pub unsafe fn as_str(&self) -> &str {
		unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.pointer.cast::<u8>(), self.length)) }
	}
}

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
		unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.pointer.cast::<u8>(), self.length)) }
	}
}

impl std::ops::Deref for CUtf8<'_> {
	type Target = str;

	fn deref(&self) -> &Self::Target {
		self.as_str()
	}
}
