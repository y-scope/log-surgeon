// use pyo3::PyErrArguments;
use std::collections::BTreeMap;

use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyIndexError;
use pyo3::exceptions::PyKeyError;
// use pyo3::conversion::IntoPyObjectExt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyUnicodeEncodeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyList;
use pyo3::types::PyListMethods;
use pyo3::types::PyString;

use crate::lexer::Fragment;
use crate::lexer::Token;
use crate::parser::LogEvent;
use crate::parser::Parser;
use crate::regex::Regex;
// use crate::regex::RegexError;
use crate::schema::Schema;

pyo3::create_exception!(logmech, LogMechException, PyRuntimeError);
pyo3::create_exception!(logmech, LogMechInvalidRegexPattern, LogMechException);

#[pyclass]
#[derive(Debug)]
struct ReaderParser {
	input: Py<PyAny>,
	schema: Schema,
	maybe_parser: Option<Parser>,
	buffer: String,
	pos: usize,
	debug: bool,
}

#[pyclass]
#[derive(Debug)]
struct PyLogEvent {
	#[pyo3(name = "header", get)]
	maybe_header: Option<Py<PyToken>>,
	#[pyo3(get)]
	tokens: Py<PyList>,
}

#[pyclass]
#[derive(Debug)]
struct PyToken {
	/// `None` for static text.
	#[pyo3(name = "rule", get)]
	maybe_rule: Option<Py<PyString>>,
	#[pyo3(name = "text", get)]
	lexeme: Py<PyString>,
	#[pyo3(get)]
	// captures: BTreeMap<String, Py<PyList>>,
	captures: Py<PyDict>,
}

#[pymethods]
impl ReaderParser {
	#[new]
	#[pyo3(signature = (debug = false))]
	fn new(debug: bool) -> Self {
		Self {
			input: Python::attach(|py| py.None()),
			schema: Schema::new(),
			maybe_parser: None,
			buffer: String::new(),
			pos: 0,
			debug,
		}
	}

	fn add_variable_pattern(&mut self, rule: &str, pattern: &str) -> PyResult<()> {
		let regex: Regex = Regex::from_pattern(pattern)
			.map_err(|err| LogMechInvalidRegexPattern::new_err(format!("Invalid pattern: {err:?}")))?;
		self.schema.add_rule(rule, regex);
		Ok(())
	}

	fn set_delimiters(&mut self, delimiters: &str) -> PyResult<()> {
		self.schema.set_delimiters(delimiters);
		Ok(())
	}

	fn compile(&mut self) -> PyResult<()> {
		self.maybe_parser = Some(Parser::new(self.schema.clone()));
		Ok(())
	}

	fn set_input_stream(&mut self, input: &Bound<'_, PyAny>) -> PyResult<()> {
		// TODO check for read method
		self.input = input.clone().unbind();
		read_from_input(input, &mut self.buffer)?;
		Ok(())
	}

	fn next_log_event(&mut self) -> PyResult<Option<PyLogEvent>> {
		if self.done() {
			return Ok(None);
		}

		let Some(lexer): Option<&mut Parser> = self.maybe_parser.as_mut() else {
			return Err(LogMechException::new_err("Parser has no input set"));
		};

		let Some(event): Option<LogEvent<'_, '_>> = lexer.next_event(&self.buffer, &mut self.pos) else {
			return Ok(None);
		};

		Python::attach(|py| {
			let tokens: Bound<'_, PyList> = PyList::empty(py);

			for token in event.tokens.into_iter() {
				tokens.append(PyToken::new(py, token)?)?;
			}

			Ok(Some(PyLogEvent {
				maybe_header: None,
				tokens: tokens.unbind(),
			}))
		})

		// Ok(Some(PyLogEvent {
		// 	header: event.maybe_header.map(|(lexeme, captures)| {
		// 		let mut combined: BTreeMap<String, Vec<String>> = BTreeMap::new();
		// 		for (key, value) in captures.into_iter() {
		// 			combined.entry(key).or_insert_with(Vec::new).push(value);
		// 		}

		// 		PyToken {
		// 			maybe_rule: Some("header".to_owned()),
		// 			lexeme,
		// 			captures: combined,
		// 		}
		// 	}),
		// 	tokens: tokens.unbind(),
		// }))
	}

	fn done(&self) -> bool {
		self.pos == self.buffer.len()
	}
}

#[pymethods]
impl PyLogEvent {
	// #[pyo3(name = "__len__")]
	// fn len(&self) -> usize {
	// 	self.tokens.len()
	// }

	// #[pyo3(name = "__getitem__")]
	// fn get_item(&self, i: usize) -> PyResult<PyToken> {
	// 	if let Some(token) = self.tokens.get(i) {
	// 		Ok(token.clone())
	// 	} else {
	// 		Err(PyIndexError::new_err(format!(
	// 			"event token index {} is out of range 0..{}",
	// 			i,
	// 			self.tokens.len()
	// 		)))
	// 	}
	// }

	#[pyo3(name = "__repr__")]
	fn repr(&self) -> String {
		format!("{self:?}")
	}
}

#[pymethods]
impl PyToken {
	// #[pyo3(name = "__getitem__")]
	// fn get_item(&self, key: &str) -> PyResult<Vec<String>> {
	// 	if let Some(captures) = self.captures.get(key) {
	// 		Ok(captures.clone())
	// 	} else {
	// 		Err(PyKeyError::new_err(format!("token has no capture {}", key)))
	// 	}
	// }

	// #[pyo3(name = "__contains__")]
	// fn contains(&self, key: &str) -> bool {
	// 	self.captures.contains_key(key)
	// }

	#[pyo3(name = "__repr__")]
	fn repr(this: PyRef<'_, Self>) -> PyResult<String> {
		Ok(format!(
			"PyToken {{ rule: {}, lexeme: {:?} }}",
			if let Some(rule) = &this.maybe_rule {
				rule.to_str(this.py())?
			} else {
				"None"
			},
			this.lexeme.to_str(this.py())?,
		))
		// format!("{self:?}")
	}
}

impl PyToken {
	fn new(py: Python<'_>, token: Token<'_, '_>) -> PyResult<Self> {
		match token {
			Token::Variable(variable) => {
				let mut captures1: BTreeMap<String, Vec<String>> = BTreeMap::new();
				for capture in variable.fragments.into_iter() {
					captures1
						.entry(capture.name.to_owned())
						.or_insert_with(Vec::new)
						.push(capture.lexeme.to_owned());
				}
				let captures2: Bound<'_, PyDict> = PyDict::new(py);
				for (key, values) in captures1.into_iter() {
					captures2.set_item(PyString::new(py, &key), PyList::new(py, values)?)?;
				}

				Ok(PyToken {
					maybe_rule: Some(PyString::new(py, variable.name).unbind()),
					lexeme: PyString::new(py, variable.lexeme).unbind(),
					captures: captures2.unbind(),
				})
			},
			Token::StaticText(lexeme) => Ok(PyToken {
				maybe_rule: None,
				lexeme: PyString::new(py, lexeme).unbind(),
				captures: PyDict::new(py).unbind(),
			}),
		}
	}
}

/// Returns 0 iff EOF.
fn read_from_input(input: &Bound<'_, PyAny>, output: &mut String) -> PyResult<usize> {
	if let Some(utf8) = python_unicode_or_bytes_as_str(input)? {
		*output += utf8;
		Ok(utf8.len())
	} else {
		// - <https://docs.python.org/3/library/io.html#io.RawIOBase.read>
		//
		// > If `size` is unspecified or -1, all bytes until EOF are read.
		// > If 0 bytes are returned, and size was not 0, this indicates end of file.
		// > If the object is in non-blocking mode and no bytes are available, `None` is returned.
		let data: Bound<'_, PyAny> = input.call_method0("read")?;

		if let Some(utf8) = python_unicode_or_bytes_as_str(&data)? {
			*output += utf8;
			Ok(utf8.len())
		} else {
			let buffer: PyBuffer<u8> = PyBuffer::<u8>::get(&data)?;

			let bytes_read: usize = buffer.len_bytes();

			if bytes_read == 0 {
				return Ok(0);
			}

			let mut tmp: Vec<u8> = vec![0; bytes_read];
			buffer.copy_to_slice(input.py(), &mut tmp[..])?;

			*output += str::from_utf8(&tmp[..])?;

			Ok(bytes_read)
		}
	}
}

fn python_unicode_or_bytes_as_str<'a>(input: &'a Bound<'_, PyAny>) -> PyResult<Option<&'a str>> {
	if let Ok(unicode) = input.cast::<PyString>() {
		Ok(Some(unicode.to_str()?))
	} else if let Ok(bytes) = input.cast::<PyBytes>() {
		match str::from_utf8(bytes.as_bytes()) {
			Ok(utf8) => Ok(Some(utf8)),
			Err(err) => Err(PyUnicodeEncodeError::new_err(err)),
		}
	} else {
		Ok(None)
	}
}

#[pymodule]
mod logmech {
	#[pymodule_export]
	use super::PyLogEvent;
	#[pymodule_export]
	use super::PyToken;
	#[pymodule_export]
	use super::ReaderParser;
}

/*
impl PyErrArguments for RegexError<'_> {
	fn arguments(self, py: Python<'_>) -> Py<PyAny> {
		// https://docs.rs/pyo3/0.27.2/src/pyo3/err/impls.rs.html#108
		format!("Invalid regex pattern: {self:?}")
			.into_pyobject(py)
			.unwrap()
			.into_any()
			.unbind()
	}
}
*/
