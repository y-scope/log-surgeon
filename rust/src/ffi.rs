// Necessary for FFI.
#![allow(clippy::box_collection)]

use std::ffi::c_char;
use std::marker::PhantomData;
use std::str::Utf8Error;

/// Represents a C `T const*` pointer + `size_t` length as a single ABI-stable value.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CArray<'lifetime, T> {
	pointer: *const T,
	length: usize,
	_lifetime: PhantomData<&'lifetime [T]>,
}

pub type CCharArray<'lifetime> = CArray<'lifetime, c_char>;

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
	pub fn null() -> Self {
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
