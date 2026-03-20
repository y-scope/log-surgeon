use log_surgeon::python_interface::stub_info;

fn main() {
	let stub = stub_info().expect("Failed to gather stub info");
	stub.generate().expect("Failed to generate type stubs");
}
