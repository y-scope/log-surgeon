macro_rules! time_this {
	($label:expr, $block:block) => {{
		use ::std::time::Instant;
		let start: Instant = Instant::now();
		let result = $block;
		::tracing::debug!("{}: {:?}", $label, start.elapsed());
		result
	}};
}

#[macro_export]
macro_rules! now {
	($var:ident) => {
		let $var: ::std::time::Instant = ::std::time::Instant::now();
	};
}

#[macro_export]
macro_rules! millis {
	($t0:ident, $t1:ident) => {{
		use ::std::time::Instant;
		let t0: Instant = $t0;
		let t1: Instant = $t1;
		t1.duration_since(t0).as_millis()
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

	#[macro_export]
	macro_rules! parser {
		($definition:expr) => {{
			use ::std::sync::Arc;
			use $crate::parsing_spec::ParsingSpec;

			let definition: &::std::primitive::str = $definition;

			let spec: ParsingSpec = $crate::spec! { definition };

			Arc::new(spec).create_parser()
		}};
	}
}
