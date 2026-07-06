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

#[macro_use]
mod test {
	#[macro_export]
	macro_rules! spec {
		($definition:expr) => {{
			use $crate::parsing_spec::ParsingSpec;
			use $crate::parsing_spec::ParsingSpecBuilder;

			let definition: &::std::primitive::str = $definition;

			// Canonicalize spec by round-tripping.
			let spec: ParsingSpec = ParsingSpecBuilder::from_parsing_spec_definition(definition)
				.unwrap()
				.build();
			let roundtrip: String = spec.to_parsing_spec_definition();
			let spec: ParsingSpec = ParsingSpecBuilder::from_parsing_spec_definition(&roundtrip)
				.unwrap()
				.build();

			spec
		}};
	}
}
