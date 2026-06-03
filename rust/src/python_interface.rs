use std::num::NonZero;

use pyo3::buffer::PyBuffer;
// use pyo3::exceptions::PyIndexError;
// use pyo3::exceptions::PyKeyError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyUnicodeEncodeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyInt;
use pyo3::types::PyList;
use pyo3::types::PyListMethods;
use pyo3::types::PySlice;
use pyo3::types::PyString;

use crate::log_event::LogEvent;
use crate::log_type::LogType;
use crate::parser::Parser;
use crate::schema::RootRule;
use crate::schema::Schema;
use crate::schema::SchemaBuilder;

pyo3::create_exception!(log_surgeon, LogSurgeonException, PyRuntimeError);
pyo3::create_exception!(log_surgeon, LogSurgeonInvalidRegexPattern, LogSurgeonException);

#[pyclass(name = "Parser")]
#[derive(Debug)]
struct PyParser {
	input: Py<PyAny>,
	schema_builder: SchemaBuilder,
	maybe_schema: Option<Schema>,
	maybe_parser: Option<Parser>,
	buffer: String,
	pos: usize,
	#[allow(unused)]
	debug: bool,
}

#[pyclass(name = "LogEvent")]
#[derive(Debug)]
struct PyLogEvent {
	#[pyo3(get)]
	log_type: Py<PyLogType>,
	#[pyo3(get)]
	message: Py<PyString>,
	#[pyo3(get, name = "leaf_captures")]
	leaf_matches: Py<PyList>,
	#[pyo3(get, name = "non_leaf_captures")]
	non_leaf_matches: Py<PyList>,
	#[pyo3(get, name = "variables")]
	root_matches: Py<PyList>,
	#[pyo3(get, name = "all_captures")]
	all_matches: Py<PyList>,
}

#[pyclass(name = "LogType", eq)]
#[derive(Debug, Eq, PartialEq)]
struct PyLogType(LogType);

#[pyclass(name = "Capture")]
#[derive(Debug)]
struct PyMatch {
	#[pyo3(get)]
	rule_id: Py<PyInt>,
	#[pyo3(get)]
	sub_rule_id: Py<PyInt>,

	#[pyo3(get)]
	parent: Option<Py<PyMatch>>,

	#[pyo3(get)]
	offsets: Py<PySlice>,

	/// Non-qualified name of this match.
	#[pyo3(get)]
	name: Py<PyString>,
	// /// Fully-qualified name of this match.
	// #[pyo3(get)]
	// qualified_name: Py<PyString>,
	/// Name of the containing root rule.
	#[pyo3(get, name = "variable_name")]
	root_rule_name: Py<PyString>,
	/// Name of this sub rule (if applicable); empty string if this is a root rule.
	#[pyo3(get, name = "capture_name")]
	sub_rule_name: Py<PyString>,

	#[pyo3(get, name = "text")]
	lexeme: Py<PyString>,
}

#[pymethods]
impl PyParser {
	#[new]
	#[pyo3(signature = (*, debug = false))]
	fn new(debug: bool) -> Self {
		Self {
			input: Python::attach(|py| py.None()),
			schema_builder: SchemaBuilder::new(),
			maybe_schema: None,
			maybe_parser: None,
			buffer: String::new(),
			pos: 0,
			debug,
		}
	}

	/// Raises an exception if `name` is empty, or `"delimiters"`
	/// (see [`SchemaBuilder::add_rule_with_priority`]).
	#[pyo3(signature = (name, pattern, *, priority=0))]
	fn add_variable_pattern(&mut self, name: &str, pattern: &str, priority: i32) -> PyResult<()> {
		self.schema_builder
			.add_rule_with_priority(priority, name, pattern)
			.map_err(|err| LogSurgeonInvalidRegexPattern::new_err(format!("invalid pattern: {err:?}")))?;
		Ok(())
	}

	/// Raises an exception if `delimiters` is empty.
	fn set_delimiters(&mut self, delimiters: &str) -> PyResult<()> {
		if delimiters.is_empty() {
			return Err(LogSurgeonException::new_err("delimiters cannot be empty"));
		}
		self.schema_builder.set_delimiters(delimiters);
		Ok(())
	}

	fn compile(&mut self) -> PyResult<()> {
		let schema: Schema = self.schema_builder.clone().build();
		self.maybe_schema = Some(schema.clone());
		self.maybe_parser = Some(Parser::new(schema));
		Ok(())
	}

	fn set_input_stream(&mut self, input: &Bound<'_, PyAny>) -> PyResult<()> {
		self.input = input.clone().unbind();
		self.pos = 0;
		self.buffer.clear();
		read_from_input(input, &mut self.buffer)?;
		Ok(())
	}

	fn next_log_event(&mut self) -> PyResult<Option<PyLogEvent>> {
		if self.done() {
			return Ok(None);
		}

		let Some(parser): Option<&mut Parser> = self.maybe_parser.as_mut() else {
			return Err(LogSurgeonException::new_err("parser has not been compiled"));
		};

		let Some(event): Option<LogEvent<'_>> = parser.next_event(&self.buffer, &mut self.pos) else {
			return Ok(None);
		};

		if self.debug {
			event.check_invariants();
		}

		Python::attach(|py| {
			let leaf_matches: Bound<'_, PyList> = PyList::empty(py);
			let non_leaf_matches: Bound<'_, PyList> = PyList::empty(py);
			let root_matches: Bound<'_, PyList> = PyList::empty(py);
			let mut all_matches: Vec<Bound<'_, PyMatch>> = Vec::new();

			let schema: &Schema = self.maybe_schema.as_ref().unwrap();

			for (i, cap) in event.all_matches.iter().enumerate() {
				let rule: &RootRule = &schema[cap.rule_idx];
				let root_rule_name: &str = &rule.name;
				let sub_rule_name: &str = rule[cap.sub_rule_id].sub_rule_name();
				let (name, parent): (&str, Option<Py<PyMatch>>) = if cap.parent_index < i {
					(sub_rule_name, Some(all_matches[cap.parent_index].clone().unbind()))
				} else {
					(root_rule_name, None)
				};
				// let qualified_name: String =
				// 	format!("{}{}", root_rule_name, rule.rule_info(cap.sub_rule_id).qualified_name());
				let name: Py<PyString> = PyString::new(py, name).unbind();
				let py_cap: Bound<'_, PyMatch> = PyMatch {
					rule_id: PyInt::new(py, u16::from(cap.rule_idx)).unbind(),
					sub_rule_id: PyInt::new(py, cap.sub_rule_id.map_or(0, NonZero::get)).unbind(),
					parent,
					offsets: PySlice::new(py, cap.range.start as isize, cap.range.end as isize, 1).unbind(),
					name,
					// qualified_name: PyString::new(py, &qualified_name).unbind(),
					root_rule_name: PyString::new(py, root_rule_name).unbind(),
					sub_rule_name: PyString::new(py, sub_rule_name).unbind(),
					lexeme: PyString::new(py, &event.message[cap.range.start..cap.range.end]).unbind(),
				}
				.into_pyobject(py)?;
				if cap.sub_rule_id.is_none() {
					root_matches.append(py_cap.clone())?;
				}
				if cap.is_leaf {
					leaf_matches.append(py_cap.clone())?;
				} else {
					non_leaf_matches.append(py_cap.clone())?;
				}
				all_matches.push(py_cap);
			}
			let all_matches: Bound<'_, PyList> = PyList::new(py, all_matches)?;

			Ok(Some(PyLogEvent {
				log_type: Py::new(py, PyLogType(event.log_type.clone()))?,
				message: PyString::new(py, event.message).unbind(),
				leaf_matches: leaf_matches.unbind(),
				non_leaf_matches: non_leaf_matches.unbind(),
				root_matches: root_matches.unbind(),
				all_matches: all_matches.unbind(),
			}))
		})
	}

	fn done(&self) -> bool {
		self.pos == self.buffer.len()
	}

	fn generate_schema_definition(&self) -> PyResult<String> {
		let Some(schema): Option<&Schema> = self.maybe_schema.as_ref() else {
			return Err(LogSurgeonException::new_err("parser has not been compiled"));
		};

		Ok(schema.to_schema_definition())
	}

	#[staticmethod]
	#[pyo3(signature = (definition, *, debug = false))]
	fn from_schema_definition(definition: &str, debug: bool) -> PyResult<Self> {
		match SchemaBuilder::from_schema_definition(definition) {
			Ok(builder) => {
				let schema: Schema = builder.build();
				Ok(Self {
					input: Python::attach(|py| py.None()),
					schema_builder: SchemaBuilder::new(),
					maybe_schema: Some(schema.clone()),
					maybe_parser: Some(Parser::new(schema)),
					buffer: String::new(),
					pos: 0,
					debug,
				})
			},
			Err(err) => Err(LogSurgeonException::new_err(format!(
				"invalid schema definition on line {}",
				err.line_offset + 1
			))),
		}
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

	#[pyo3(name = "__str__")]
	fn to_string<'py>(this: PyRef<'py, Self>) -> Py<PyString> {
		this.message.clone_ref(this.py())
	}
}

#[pymethods]
impl PyLogType {
	#[pyo3(name = "__str__")]
	fn as_str(&self) -> &str {
		self.0.as_str()
	}
}

#[pymethods]
impl PyMatch {
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
	fn repr(&self) -> String {
		format!("{self:?}")
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
mod log_surgeon_ffi {
	#[pymodule_export]
	use super::PyLogEvent;
	#[pymodule_export]
	use super::PyLogType;
	#[pymodule_export]
	use super::PyMatch;
	#[pymodule_export]
	use super::PyParser;
	use super::*;

	#[pyfunction]
	fn enable_tracing() {
		crate::enable_tracing();
	}
}
