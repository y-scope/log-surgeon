macro_rules! time_this {
	($label:expr, $block:block) => {{
		use ::std::time::Instant;
		let start: Instant = Instant::now();
		let result = $block;
		println!("{}: {:?}", $label, start.elapsed());
		result
	}};
}

macro_rules! now {
	($var:ident) => {
		let $var: ::std::time::Instant = ::std::time::Instant::now();
	};
}

macro_rules! how_long {
	($var:ident) => {{
		use ::std::time::Instant;
		let var: Instant = $var;
		var.elapsed()
	}};
}

#[cfg(test)]
#[macro_use]
mod test {
	macro_rules! schema {
		($definition:expr) => {{
			use $crate::schema::Schema;

			let definition: &::std::primitive::str = $definition;

			// Canonicalize schema by round-tripping.
			let schema: Schema = Schema::from_schema_definition(definition).unwrap();
			let roundtrip: String = schema.to_schema_definition();
			let schema: Schema = Schema::from_schema_definition(&roundtrip).unwrap();

			schema
		}};
	}
}
