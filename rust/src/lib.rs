// Internal wip code that has been requested to be made public... please forgive me.
// #![allow(unused)]

#[macro_use(debug)]
extern crate tracing;

pub mod dfa;
pub mod interval_tree;
pub mod lexer;
pub mod log_type;
pub mod nfa;
pub mod parser;
pub mod regex;
pub mod schema;

pub mod c_interface;
#[cfg(feature = "python")]
pub mod python_interface;
