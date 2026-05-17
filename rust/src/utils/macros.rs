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
