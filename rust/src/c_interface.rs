use std::ffi::c_char;
use std::marker::PhantomData;
use std::str::Utf8Error;

use crate::lexer::Lexer;
use crate::lexer::LogComponent;
use crate::regex::Regex;
use crate::schema::Schema;

#[repr(C)]
pub struct CSlice<'lifetime, T> {
	pointer: *const T,
	length: usize,
	_lifetime: PhantomData<&'lifetime [T]>,
}

pub type CStringView<'lifetime> = CSlice<'lifetime, c_char>;

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_c_string_view<'unknown>(pointer: *const c_char) -> CSlice<'unknown, c_char> {
	CSlice {
		pointer,
		length: unsafe { libc::strlen(pointer) },
		_lifetime: PhantomData,
	}
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
unsafe extern "C" fn clp_log_mechanic_schema_set_delimiters(schema: &mut Schema, delimiters: CStringView<'_>) {
	schema.set_delimiters(delimiters.as_utf8().unwrap());
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_schema_add_rule(
	schema: &mut Schema,
	name: CStringView<'_>,
	pattern: CStringView<'_>,
) {
	let name: &str = name.as_utf8().unwrap();
	let pattern: &str = pattern.as_utf8().unwrap();
	let regex: Regex = Regex::from_pattern(pattern).unwrap();
	schema.add_rule(name, regex);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_lexer_new<'schema>(schema: &'schema Schema) -> Box<Lexer<'schema>> {
	let lexer: Lexer<'_> = Lexer::new(schema).unwrap();
	Box::new(lexer)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_lexer_delete(lexer: Box<Lexer<'_>>) {
	std::mem::drop(lexer);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_lexer_next_token<'schema, 'lexer>(
	lexer: &'lexer mut Lexer<'schema>,
	input: CStringView<'_>,
	pos: &mut usize,
	log_component: &mut LogComponent<'schema, 'lexer>,
) -> bool {
	if let Some(component) = lexer.next_token(input.as_utf8().unwrap(), pos) {
		*log_component = component;
		true
	} else {
		false
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
