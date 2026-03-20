use log_surgeon::python_interface::stub_info;
use std::fs;
use std::path::Path;

/// Replace `old` with `new` in `s`, panicking if `old` is not found.
fn refine(s: String, old: &str, new: &str) -> String {
	assert!(
		s.contains(old),
		"type refinement failed: pattern not found in generated stubs.\n\
		 Expected: {old}\n\
		 pyo3-stub-gen output may have changed."
	);
	s.replace(old, new)
}

fn main() {
	let stub = stub_info().expect("Failed to gather stub info");
	stub.generate().expect("Failed to generate type stubs");

	// pyo3-stub-gen generates imprecise types for Py<PyList>, Py<PyDict>, and
	// &Bound<PyAny> fields. Apply known type refinements to the generated stubs.
	let path = Path::new("python/log_surgeon/__init__.pyi");
	let content = fs::read_to_string(path).expect("Failed to read generated stubs");
	let refined = content;
	let refined = refine(
		refined,
		"def set_input_stream(self, input: typing.Any)",
		"def set_input_stream(self, input: str | bytes | typing.IO[bytes])",
	);
	let refined = refine(
		refined,
		"def variables(self) -> list:",
		"def variables(self) -> list[Variable]:",
	);
	let refined = refine(
		refined,
		"def captures(self) -> dict:",
		"def captures(self) -> dict[str, list[str]]:",
	);
	fs::write(path, refined).expect("Failed to write refined stubs");
}
