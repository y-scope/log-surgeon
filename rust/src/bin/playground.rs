#![allow(unused)]

use log_mechanic::dfa::*;
use log_mechanic::nfa::*;
use log_mechanic::regex::*;
use log_mechanic::schema::*;

fn main() {
	// let r: Regex = Regex::from_pattern("(?<bar>[a-z]*)|(?<foo>b)+").unwrap();
	// let r: Regex = Regex::from_pattern("(?<foo>bbb)*|(?<bar>b*)").unwrap();
	let r: Regex = Regex::from_pattern("(?<foo>b)|(?<bar>b)").unwrap();
	let r: Regex = Regex::from_pattern("(?<bar>b*)|(?<foo>(bb)*)").unwrap();
	let r: Regex = Regex::from_pattern("(?<foo>(bb)*)|(?<bar>b*)").unwrap();
	let r: Regex = Regex::from_pattern("(?<foo>b*)|(?<bar>b)*").unwrap();
	let r: Regex = Regex::from_pattern("(?<bar>b*)|(?<foo>b)*").unwrap();
	let mut schema: Schema = Schema::new();
	schema.add_rule("hello", r);
	let dfa: Dfa = Dfa::for_schema(&schema);
	let b: bool = dfa
		.simulate_with_captures("bbbbbb", |var, lexeme| {
			println!("got {var:?}: {lexeme:?}");
		})
		.is_ok();
	println!("matched: {b}");
}

fn main3() {
	let r: Regex = Regex::from_pattern("((?<foo>xyz)|(?<bar>xya))+").unwrap();
	let mut schema: Schema = Schema::new();
	schema.add_rule("hello", r);
	let dfa: Dfa = Dfa::for_schema(&schema);
	let b: bool = dfa
		.simulate_with_captures("xyaxyzxya", |var, lexeme| {
			println!("got {var:?}: {lexeme:?}");
		})
		.is_ok();
	println!("matched: {b}");
}

fn main2() {
	// tracing_subscriber::fmt()
	// 	.pretty()
	// 	.with_file(true)
	// 	.with_line_number(true)
	// 	.with_span_events(
	// 		tracing_subscriber::fmt::format::FmtSpan::ENTER | tracing_subscriber::fmt::format::FmtSpan::EXIT,
	// 	)
	// 	.init();
	{
		let r: Regex = Regex::from_pattern("0((?<foobar>1(2[a-zA-Z])*)*|(?<baz>xyz))*world").unwrap();
		// let r: Regex = Regex::from_pattern("a*").unwrap();
		// let r: Regex = Regex::from_pattern("[^a-z]|(abc){3,5}").unwrap();
		// let r: Regex = Regex::from_pattern("0(xyz)*world").unwrap();
		// let r: Regex = Regex::from_pattern("(?<foo>a)*").unwrap();
		// let r: Regex = Regex::from_pattern("((?<foo>a))*").unwrap();
		// let r: Regex = Regex::from_pattern("(1(?<a>a)|b)*").unwrap();
		dbg!(&r);
		let mut schema: Schema = Schema::new();
		schema.add_rule("hello", r);
		let nfa: Nfa = Nfa::for_schema(&schema);
		println!("nfa is {nfa:#?}");
		// let b: bool = nfa.simulate("aabba").is_some();
		// assert!(b);
		// return;
		// let b: bool = nfa.simulate("012a2b2cworld").is_some();
		// assert!(b);
		let dfa: Dfa = Dfa::for_schema(&schema);
		dbg!(&dfa);
		// let b: bool = dfa.simulate("aaaaa");
		// println!("matched: {b}");
		let b: bool = dfa.simulate("012a2b2c12z12zxyzxyzxyzworld");
		// let b: bool = dfa.simulate("1a1ab1a");
		println!("matched: {b}");
		// let b: bool = dfa.simulate("0xyzworld");
		// println!("matched: {b}");
		// let b: bool = dfa.simulate("0world");
		// println!("matched: {b}");
		// let b: bool = dfa.simulate("0xyz12a2b2cxyzworld");
		// println!("matched: {b}");
	}
	// {
	// 	let r: Regex = Regex::from_pattern("0(?<foo>xyz)*xy").unwrap();
	// 	let mut schema: Schema = Schema::new();
	// 	schema.add_rule("hello", r);
	// 	let nfa: Nfa<'_> = Nfa::for_schema(&schema).unwrap();
	// 	let b: bool = nfa.simulate("0xyzxy").is_some();
	// 	println!("matched: {b}");
	// }
	// {
	// 	let r: Regex = Regex::from_pattern("((?<foo>foo)|(?<bar>bar))*").unwrap();
	// 	let mut schema: Schema = Schema::new();
	// 	schema.add_rule("hello", r);
	// 	let nfa: Nfa<'_> = Nfa::for_schema(&schema).unwrap();
	// 	let b: bool = nfa.simulate("foofoofoo").is_some();
	// 	println!("matched: {b}");
	// }

	// 	let dfa: Dfa<'_> = nfa.determinization();
	// 	dbg!(&dfa);
	// 	let b: bool = dfa.simulate("foofoofoo");
	// 	println!("matched: {b}");
	// }
}
