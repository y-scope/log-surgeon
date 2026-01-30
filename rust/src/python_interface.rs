// use pyo3::PyErrArguments;
use std::collections::BTreeMap;

use pyo3::buffer::PyBuffer;
// use pyo3::conversion::IntoPyObjectExt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyUnicodeEncodeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyString;

use crate::lexer::Fragment;
use crate::lexer::Lexer;
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
	maybe_lexer: Option<Lexer>,
	buffer: String,
	pos: usize,
	debug: bool,
}

#[pyclass]
#[derive(Debug)]
struct LogEvent {
	#[pyo3(get, set)]
	static_text: String,
	#[pyo3(get, set)]
	variable: Option<Variable>,
}

#[pyclass]
#[derive(Debug, Clone)]
struct Variable {
	#[pyo3(get)]
	name: String,
	#[pyo3(get)]
	text: String,
	#[pyo3(get)]
	captures: BTreeMap<String, Vec<String>>,
}

#[pymethods]
impl ReaderParser {
	#[new]
	#[pyo3(signature = (debug = false))]
	fn new(debug: bool) -> Self {
		Self {
			input: Python::attach(|py| py.None()),
			schema: Schema::new(),
			maybe_lexer: None,
			buffer: String::new(),
			pos: 0,
			debug,
		}
	}

	fn add_var(&mut self, rule: &str, pattern: &str) -> PyResult<()> {
		let regex: Regex = Regex::from_pattern(pattern)
			.map_err(|err| LogMechInvalidRegexPattern::new_err(format!("Invalid pattern: {err:?}")))?;
		self.schema.add_rule(rule, regex);
		Ok(())
	}

	fn compile(&mut self) -> PyResult<()> {
		self.maybe_lexer = Some(
			Lexer::new(&self.schema)
				.map_err(|err| LogMechInvalidRegexPattern::new_err(format!("Ill-formed pattern: {err:?}")))?,
		);
		Ok(())
	}

	fn set_input_stream(&mut self, input: &Bound<'_, PyAny>) -> PyResult<()> {
		// TODO check for read method
		self.input = input.clone().unbind();
		read_from_input(input, &mut self.buffer)?;
		Ok(())
	}

	fn next_log_event(&mut self) -> PyResult<Option<LogEvent>> {
		if self.done() {
			return Ok(None);
		}

		let Some(lexer): Option<&mut Lexer> = self.maybe_lexer.as_mut() else {
			return Err(LogMechException::new_err("Parser has no input set"));
		};

		let mut captures: BTreeMap<String, Vec<String>> = BTreeMap::new();

		let fragment: Fragment<'_> = lexer.next_fragment(&self.buffer, &mut self.pos, |variable, lexeme| {
			captures
				.entry(variable.name.clone())
				.or_insert_with(Vec::new)
				.push(lexeme.to_owned());
		});

		// We checked for "not done" input above;
		// at least one of static text or the variable should be non-empty.
		assert!(!fragment.static_text.is_empty() || (fragment.rule != 0));

		Ok(Some(LogEvent {
			static_text: fragment.static_text.to_owned(),
			variable: if fragment.rule != 0 {
				Some(Variable {
					name: self.schema.rules()[fragment.rule].name.clone(),
					text: fragment.lexeme.to_owned(),
					captures,
				})
			} else {
				None
			},
		}))
	}

	fn done(&self) -> bool {
		self.pos == self.buffer.len()
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
	use super::LogEvent;
	#[pymodule_export]
	use super::ReaderParser;
	#[pymodule_export]
	use super::Variable;
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
