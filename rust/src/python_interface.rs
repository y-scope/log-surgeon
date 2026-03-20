use std::collections::BTreeMap;
use std::convert::Infallible;

use pyo3::buffer::PyBuffer;
// use pyo3::exceptions::PyIndexError;
// use pyo3::exceptions::PyKeyError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyUnicodeEncodeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyList;
use pyo3::types::PyListMethods;
use pyo3::types::PyString;
use pyo3_stub_gen::derive::*;
use pyo3_stub_gen::define_stub_info_gatherer;

use crate::log_event::LogEvent;
use crate::log_event::Variable;
use crate::log_type::LogType;
use crate::parser::Parser;
use crate::regex::Regex;
use crate::schema::Schema;

pyo3::create_exception!(log_surgeon, LogSurgeonException, PyRuntimeError);
pyo3::create_exception!(log_surgeon, LogSurgeonInvalidRegexPattern, LogSurgeonException);

/// High-performance log parser using DFA-based pattern matching.
#[gen_stub_pyclass]
#[pyclass(name = "Parser")]
#[derive(Debug)]
struct PyParser {
	input: Py<PyAny>,
	schema: Schema,
	maybe_parser: Option<Parser>,
	buffer: String,
	pos: usize,
	#[allow(unused)]
	debug: bool,
}

/// A parsed log event.
// Fields are exposed via explicit `#[getter]` methods (not `#[pyo3(get)]`) so
// that pyo3-stub-gen can apply type overrides for the generated `.pyi` stubs.
// Doc comments on those getters become the Python-side docstrings.
#[gen_stub_pyclass]
#[pyclass(name = "LogEvent")]
#[derive(Debug)]
struct PyLogEvent {
	log_type: Py<PyLogType>,
	variables: Py<PyList>,
	message: Py<PyString>,
}

/// A log type template showing the structure of a log event.
#[gen_stub_pyclass]
#[pyclass(name = "LogType", eq)]
#[derive(Debug, Eq, PartialEq)]
struct PyLogType(LogType);

/// A matched variable within a log event.
#[gen_stub_pyclass]
#[pyclass(name = "Variable")]
#[derive(Debug)]
struct PyVariable {
	name: Py<PyString>,
	lexeme: Py<PyString>,
	captures: Py<PyDict>,
}

#[gen_stub_pymethods]
#[pymethods]
impl PyParser {
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

	/// Add a named pattern with ``(?<capture_name>...)`` groups.
	///
	/// Higher priority patterns are matched first. Default is 0.
	#[pyo3(signature = (name, pattern, *, priority=0))]
	fn add_variable_pattern(&mut self, name: &str, pattern: &str, priority: i32) -> PyResult<()> {
		let regex: Regex = Regex::from_pattern(pattern)
			.map_err(|err| LogSurgeonInvalidRegexPattern::new_err(format!("invalid pattern: {err:?}")))?;
		let Ok(_): Result<(), Infallible> = self.schema.add_rule_with_priority(priority, name, regex);
		Ok(())
	}

	/// Set token boundary characters. Raises if empty.
	fn set_delimiters(&mut self, delimiters: &str) -> PyResult<()> {
		if delimiters.is_empty() {
			return Err(LogSurgeonException::new_err("delimiters cannot be empty"));
		}
		self.schema.set_delimiters(delimiters);
		Ok(())
	}

	/// Compile patterns into the matching engine. Must be called before parsing.
	fn compile(&mut self) -> PyResult<()> {
		self.maybe_parser = Some(Parser::new(self.schema.clone()));
		Ok(())
	}

	/// Set the input to parse (string, bytes, or readable file object).
	fn set_input_stream(
		&mut self,
		#[gen_stub(override_type(type_repr = "str | bytes | typing.IO[bytes]"))]
		input: &Bound<'_, PyAny>,
	) -> PyResult<()> {
		self.input = input.clone().unbind();
		self.pos = 0;
		self.buffer.clear();
		read_from_input(input, &mut self.buffer)?;
		Ok(())
	}

	/// Get the next parsed event, or ``None`` at end of input.
	fn next_log_event(&mut self) -> PyResult<Option<PyLogEvent>> {
		if self.done() {
			return Ok(None);
		}

		let Some(lexer): Option<&mut Parser> = self.maybe_parser.as_mut() else {
			return Err(LogSurgeonException::new_err("parser has not been compiled"));
		};

		let Some(event): Option<LogEvent<'_>> = lexer.next_event(&self.buffer, &mut self.pos) else {
			return Ok(None);
		};

		Python::attach(|py| {
			let variables: Bound<'_, PyList> = PyList::empty(py);

			for variable in event.variables.iter() {
				variables.append(PyVariable::new(py, variable)?)?;
			}

			Ok(Some(PyLogEvent {
				log_type: Py::new(py, PyLogType(event.log_type.clone()))?,
				variables: variables.unbind(),
				message: PyString::new(py, event.message).unbind(),
			}))
		})
	}

	/// Check if all input has been consumed.
	fn done(&self) -> bool {
		self.pos == self.buffer.len()
	}
}

#[gen_stub_pymethods]
#[pymethods]
impl PyLogEvent {
	/// Template with ``%rule_name%`` placeholders for matched variables.
	#[getter]
	fn log_type(&self) -> Py<PyLogType> {
		Python::attach(|py| self.log_type.clone_ref(py))
	}

	/// List of variables that matched in this event.
	#[getter]
	#[gen_stub(override_return_type(type_repr = "list[Variable]"))]
	fn variables(&self) -> Py<PyList> {
		Python::attach(|py| self.variables.clone_ref(py))
	}

	/// The original text of the log event.
	#[getter]
	fn message(&self) -> Py<PyString> {
		Python::attach(|py| self.message.clone_ref(py))
	}

	#[pyo3(name = "__str__")]
	fn to_string<'py>(this: PyRef<'py, Self>) -> Py<PyString> {
		this.message.clone_ref(this.py())
	}
}

#[gen_stub_pymethods]
#[pymethods]
impl PyLogType {
	#[pyo3(name = "__str__")]
	fn as_str(&self) -> &str {
		self.0.as_str()
	}
}

#[gen_stub_pymethods]
#[pymethods]
impl PyVariable {
	/// The rule name passed to ``add_variable_pattern()``.
	#[getter]
	fn name(&self) -> Py<PyString> {
		Python::attach(|py| self.name.clone_ref(py))
	}

	/// The matched text.
	#[getter]
	fn text(&self) -> Py<PyString> {
		Python::attach(|py| self.lexeme.clone_ref(py))
	}

	/// Capture group names mapped to their matched values.
	#[getter]
	#[gen_stub(override_return_type(type_repr = "dict[str, list[str]]"))]
	fn captures(&self) -> Py<PyDict> {
		Python::attach(|py| self.captures.clone_ref(py))
	}

	#[pyo3(name = "__repr__")]
	fn repr(&self) -> String {
		format!("{self:?}")
	}
}

impl PyVariable {
	fn new(py: Python<'_>, variable: &Variable<'_>) -> PyResult<Self> {
		let mut captures1: BTreeMap<String, Vec<String>> = BTreeMap::new();
		for capture in variable.captures.iter() {
			captures1
				.entry(capture.name.to_owned())
				.or_insert_with(Vec::new)
				.push(capture.lexeme.to_owned());
		}
		let captures2: Bound<'_, PyDict> = PyDict::new(py);
		for (key, values) in captures1.into_iter() {
			captures2.set_item(PyString::new(py, &key), PyList::new(py, values)?)?;
		}

		Ok(Self {
			name: PyString::new(py, variable.name).unbind(),
			lexeme: PyString::new(py, variable.lexeme).unbind(),
			captures: captures2.unbind(),
		})
	}
}

/// Returns 0 iff EOF.
fn read_from_input(input: &Bound<'_, PyAny>, output: &mut String) -> PyResult<usize> {
	if let Some(utf8) = python_unicode_or_bytes_as_str(input)? {
		*output += utf8;
		Ok(utf8.len())
	} else {
		let id_read: &Bound<'_, PyString> = pyo3::intern!(input.py(), "read");
		if !input.hasattr(id_read)? {
			return Err(LogSurgeonException::new_err(
				"input stream must be a string, bytes, or a `read`able object",
			));
		}

		// - <https://docs.python.org/3/library/io.html#io.RawIOBase.read>
		//
		// > If `size` is unspecified or -1, all bytes until EOF are read.
		// > If 0 bytes are returned, and size was not 0, this indicates end of file.
		// > If the object is in non-blocking mode and no bytes are available, `None` is returned.
		let data: Bound<'_, PyAny> = input.call_method0(id_read)?;

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
mod log_surgeon {
	#[pymodule_export]
	use super::PyLogEvent;
	#[pymodule_export]
	use super::PyLogType;
	#[pymodule_export]
	use super::PyParser;
	#[pymodule_export]
	use super::PyVariable;
}

define_stub_info_gatherer!(stub_info);
