use std::error::Error;
use std::path::Path;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
	let root_dir: PathBuf = std::env::current_dir().unwrap();

	// Ignore error; don't choke rustfmt/rust-analyzer/rustc just because there's a syntax error.
	let _ = generate_c_bindings(&root_dir);

	Ok(())
}

fn generate_c_bindings(root_dir: &Path) -> Result<(), Box<dyn Error>> {
	cbindgen::Builder::new()
		.with_config(cbindgen::Config::from_file("cbindgen.toml")?)
		.with_crate(&root_dir)
		.generate()?
		.write_to_file(
			root_dir
				.join("cpp_ffi")
				.join("log_surgeon")
				.join("generated_bindings.hpp"),
		);
	Ok(())
}
