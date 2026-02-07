use std::ffi::c_char;
use std::marker::PhantomData;
use std::str::Utf8Error;

use crate::lexer::Capture;
use crate::lexer::Fragment;
use crate::lexer::Lexer;
use crate::regex::Regex;
use crate::schema::Schema;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CSlice<'lifetime, T> {
	pointer: *const T,
	length: usize,
	_lifetime: PhantomData<&'lifetime [T]>,
}

pub type CStringView<'lifetime> = CSlice<'lifetime, c_char>;

#[repr(C)]
pub struct CLogFragment<'schema, 'input, 'buffer> {
	/// `0` iff no variable found (static text until end of input).
	pub rule: usize,
	/// Start of variable (if found).
	pub start: *const u8,
	/// End of variable (if found).
	pub end: *const u8,
	/// Pointer to an array of captures (if variable found).
	pub captures: *const Capture<'schema, 'input>,
	/// Number of captures.
	pub captures_count: usize,
	/// Indicates that `captures` points into (borrows from) some external `'buffer`.
	pub _captures_lifetime: PhantomData<&'buffer [Capture<'schema, 'input>]>,
}

#[unsafe(no_mangle)]
extern "C" fn clp_log_mechanic_set_debug(enable: bool) {
	crate::debug::set_debug(enable);
}

#[unsafe(no_mangle)]
extern "C" fn clp_log_mechanic_schema_new() -> Box<Schema> {
	Box::new(Schema::new())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_delete(schema: Box<Schema>) {
	std::mem::drop(schema);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_set_delimiters(schema: &mut Schema, delimiters: CStringView<'_>) -> bool {
	let Ok(delimiters) = delimiters.as_utf8() else {
		return false;
	};
	schema.set_delimiters(delimiters);
	true
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_add_rule(
	schema: &mut Schema,
	name: CStringView<'_>,
	pattern: CStringView<'_>,
) -> bool {
	let Ok(name) = name.as_utf8() else {
		return false;
	};
	let Ok(pattern) = pattern.as_utf8() else {
		return false;
	};
	let Ok(regex) = Regex::from_pattern(pattern) else {
		return false;
	};
	schema.add_rule(name, regex);
	true
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_rule_count(schema: &Schema) -> usize {
	schema.rules().len()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_rule_name<'schema>(schema: &'schema Schema, index: usize) -> CStringView<'schema> {
	CStringView::from_utf8(&schema.rules()[index].name)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_lexer_new<'schema, 'a>(schema: &'schema Schema) -> *mut Lexer<'schema, 'a> {
	match Lexer::new(schema) {
		Ok(lexer) => Box::into_raw(Box::new(lexer)),
		Err(_) => std::ptr::null_mut(),
	}
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_lexer_delete<'a>(lexer: *mut Lexer<'_, 'a>) {
	if !lexer.is_null() {
		drop(unsafe { Box::from_raw(lexer) });
	}
}

/// Very unsafe!
///
/// The returned [`CLogFragment`] includes a hidden exclusive borrow of `lexer`
/// (it contains a pointer into an internal buffer of `lexer`),
/// so it is no longer valid (you must not touch it) after any subsequent borrow of `lexer`
/// (i.e. this borrow has ended).
#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_lexer_next_fragment<'schema, 'lexer, 'input>(
	lexer: &'lexer mut Lexer<'schema, 'input>,
	input: CStringView<'input>,
	pos: &mut usize,
) -> CLogFragment<'schema, 'input, 'lexer> {
	let fragment: Fragment<'_, '_, '_> = lexer.next_fragment(input.as_utf8().unwrap(), pos);
	CLogFragment {
		rule: fragment.rule,
		start: fragment.lexeme.as_bytes().as_ptr_range().start,
		end: fragment.lexeme.as_bytes().as_ptr_range().end,
		captures: fragment.captures.as_ptr(),
		captures_count: fragment.captures.len(),
		_captures_lifetime: PhantomData,
	}
}

impl<'lifetime, T> CSlice<'lifetime, T> {
	pub fn as_slice(&self) -> &'lifetime [T] {
		unsafe { std::slice::from_raw_parts(self.pointer, self.length) }
	}
}

impl<'lifetime> CStringView<'lifetime> {
	pub fn from_utf8(utf8: &'lifetime str) -> Self {
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

#[cfg(test)]
mod test {
	use super::*;
	use crate::regex::Regex;

	#[test]
	fn basic() {
		let mut schema: Schema = Schema::new();
		schema.set_delimiters(" ");
		schema.add_rule("hello", Regex::from_pattern("hello world").unwrap());
		schema.add_rule("bye", Regex::from_pattern("goodbye").unwrap());

		let mut lexer: Lexer<'_, '_> = Lexer::new(&schema).unwrap();
		let input: CStringView<'_> = CStringView::from_utf8("hello world goodbye hello world  goodbye  ");
		let mut pos: usize = 0;

		unsafe {
			assert_eq!(
				clp_log_mechanic_lexer_next_fragment(&mut lexer, input, &mut pos).rule,
				1
			);
			assert_eq!(
				clp_log_mechanic_lexer_next_fragment(&mut lexer, input, &mut pos).rule,
				2
			);
			assert_eq!(
				clp_log_mechanic_lexer_next_fragment(&mut lexer, input, &mut pos).rule,
				1
			);
			assert_eq!(
				clp_log_mechanic_lexer_next_fragment(&mut lexer, input, &mut pos).rule,
				2
			);
			assert_eq!(
				clp_log_mechanic_lexer_next_fragment(&mut lexer, input, &mut pos).rule,
				0
			);
		}
		assert_eq!(pos, input.length);
	}
}
