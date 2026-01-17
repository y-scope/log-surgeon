use std::ffi::c_char;
use std::marker::PhantomData;
use std::str::Utf8Error;

use crate::log_event::LogComponent;
use crate::nfa::Nfa;
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
unsafe extern "C" fn clp_log_mechanic_nfa_for_schema<'schema>(schema: &'schema Schema) -> Option<Box<Nfa<'schema>>> {
	let nfa: Nfa<'_> = Nfa::for_schema(schema).ok()?;
	Some(Box::new(nfa))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_nfa_delete(nfa: Box<Nfa<'_>>) {
	std::mem::drop(nfa);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_nfa_debug(nfa: &Nfa<'_>) {
	println!("nfa: {nfa:#?}");
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_parse<'schema, 'input>(
	nfa: &Nfa<'schema>,
	input: *const u8,
	len: usize,
	pos: &mut usize,
) -> Option<Box<LogComponent<'schema, 'input>>> {
	let bytes: &[u8] = unsafe { std::slice::from_raw_parts(input, len) };
	let bytes: &[u8] = &bytes[*pos..];
	let input: &str = str::from_utf8(bytes).unwrap();
	let (component, consumed): (LogComponent<'_, '_>, usize) = nfa.simulate(input)?;
	*pos += consumed;
	Some(Box::new(component))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_component_delete(component: Box<LogComponent<'_, '_>>) {
	std::mem::drop(component);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_component_text(component: &LogComponent<'_, '_>, len: &mut usize) -> *const u8 {
	*len = component.full_text.len();
	component.full_text.as_ptr()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_component_matches_count(component: &LogComponent<'_, '_>) -> usize {
	component.matches.len()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_mechanic_component_matches_get(
	component: &LogComponent<'_, '_>,
	i: usize,
	rule: &mut usize,
	name: &mut *const u8,
	name_len: &mut usize,
	start: &mut usize,
	end: &mut usize,
) {
	*rule = component.matches[i].0;
	*name = component.matches[i].1.as_ptr();
	*name_len = component.matches[i].1.len();
	*start = component.matches[i].2;
	*end = component.matches[i].3;
}

impl<'lifetime, T> CSlice<'lifetime, T> {
	fn as_slice(&self) -> &'lifetime [T] {
		unsafe { std::slice::from_raw_parts(self.pointer, self.length) }
	}
}

impl<'lifetime> CSlice<'lifetime, c_char> {
	fn as_utf8(&self) -> Result<&'lifetime str, Utf8Error> {
		let bytes: &[u8] = unsafe { std::slice::from_raw_parts(self.pointer.cast::<u8>(), self.length) };
		str::from_utf8(bytes)
	}
}
