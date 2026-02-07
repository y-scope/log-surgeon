use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_debug(enabled: bool) {
	DEBUG_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_debug() -> bool {
	DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Prints to stdout only when debug mode is enabled.
#[macro_export]
macro_rules! debug_println {
	($($arg:tt)*) => {
		if $crate::debug::is_debug() {
			println!($($arg)*);
		}
	};
}
