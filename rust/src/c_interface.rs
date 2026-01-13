use crate::log_event::LogComponent;
use crate::nfa::Nfa;
use crate::regex::Regex;
use crate::schema::Schema;

#[unsafe(no_mangle)]
extern "C" fn clp_log_surgeon_schema_new() -> Box<Schema> {
	Box::new(Schema::new())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_schema_delete(schema: Box<Schema>) {
	std::mem::drop(schema);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_schema_add_rule(
	schema: &mut Schema,
	name: *const u8,
	len: usize,
	regex: Box<Regex>,
) {
	let bytes: &[u8] = unsafe { std::slice::from_raw_parts(name, len) };
	let name: &str = str::from_utf8(bytes).unwrap();
	schema.add_rule(name, *regex);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_regex_from_pattern(pattern: *const u8, len: usize) -> Option<Box<Regex>> {
	let bytes: &[u8] = unsafe { std::slice::from_raw_parts(pattern, len) };
	let pattern: &str = str::from_utf8(bytes).unwrap();
	let regex: Regex = Regex::from_pattern(pattern).ok()?;
	Some(Box::new(regex))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_regex_delete(regex: Box<Regex>) {
	std::mem::drop(regex);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_nfa_for_schema(schema: &Schema) -> Option<Box<Nfa<'_>>> {
	let nfa: Nfa<'_> = Nfa::for_schema(schema).ok()?;
	Some(Box::new(nfa))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_nfa_delete(nfa: Box<Nfa<'_>>) {
	std::mem::drop(nfa);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_nfa_debug(nfa: &Nfa<'_>) {
	println!("nfa: {nfa:#?}");
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_parse<'schema, 'input>(
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
unsafe extern "C" fn clp_log_surgeon_component_delete(component: Box<LogComponent<'_, '_>>) {
	std::mem::drop(component);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_component_text(component: &LogComponent<'_, '_>, len: &mut usize) -> *const u8 {
	*len = component.full_text.len();
	component.full_text.as_ptr()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_component_matches_count(component: &LogComponent<'_, '_>) -> usize {
	component.matches.len()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn clp_log_surgeon_component_matches_get(
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
