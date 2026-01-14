use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
	let root_dir: PathBuf = std::env::current_dir().unwrap();

	cbindgen::Builder::new()
		.with_config(cbindgen::Config::from_file("cbindgen.toml")?)
		.with_crate(&root_dir)
		.generate()?
		.write_to_file(root_dir.join("include").join("log_mechanic.h"));

	Ok(())
}
