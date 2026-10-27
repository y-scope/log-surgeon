use std::error::Error;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
	// By default, Cargo re-runs the build script (and consequently the build)
	// if any file within the package is changed.
	//
	// <https://doc.rust-lang.org/cargo/reference/build-scripts.html#change-detection>
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=src");
	println!("cargo:rerun-if-changed=cbindgen.toml");

	let root_dir: PathBuf = std::env::current_dir().unwrap();

	// Ignore error; don't choke rustfmt/rust-analyzer/rustc just because there's a syntax error.
	if let Err(err) = generate_cxx_bindings(&root_dir) {
		eprintln!("[build.rs] error generating C bindings: {err}");
	}

	// Try to format the generated CXX bindings;
	// ignore errors so it doesn't choke the build.
	// let _: Result<_, _> = Command::new("task")
	// 	.arg("--force")
	// 	.arg("lint:fix-cpp-format-log-surgeon")
	// 	.status();
	let _: Result<_, _> = Command::new("clang-format")
		.arg("-i")
		.arg(
			root_dir
				.join("cxx")
				.join("log_surgeon")
				.join("generated_bindings.hpp"),
		)
		.status();

	Ok(())
}

fn generate_cxx_bindings(root_dir: &Path) -> Result<(), Box<dyn Error>> {
	cbindgen::Builder::new()
		.with_config(cbindgen::Config::from_file("cbindgen.toml")?)
		.with_crate(root_dir)
		.generate()?
		.write_to_file(
			root_dir
				.join("cxx")
				.join("log_surgeon")
				.join("generated_bindings.hpp"),
		);
	Ok(())
}
