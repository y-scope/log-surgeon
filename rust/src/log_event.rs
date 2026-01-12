pub struct LogComponent<'schema, 'input> {
	pub full_text: &'input str,
	pub matches: Vec<(usize, &'schema str, usize, usize)>,
}
