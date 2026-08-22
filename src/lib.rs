//! # update-checker
//!
//! A Rust library that checks for crate updates.
//!
//! **update-checker** only checks crates that are publicly listed on
//! [crates.io](https://crates.io).
//!
//! # Quick Start
//!
//! ```shell
//! cargo add update-checker
//! ```
//!
//! # Usage
//!
//! The easiest way to use this crate is with the [`check`] function:
//!
//! ```no_run
//! fn main() {
//!     // Check for updates at startup
//!     update_checker::check(
//!         env!("CARGO_PKG_NAME"),
//!         env!("CARGO_PKG_VERSION"),
//!         false, // use cache
//!     );
//!
//!     println!("Hello, world!");
//! }
//! ```
//!
//! If an update is available, it prints to stderr:
//!
//! ```text
//! Version 1.0.0 of my-tool is outdated. Version 1.2.0 was released 3 days ago.
//! ```
//!
//! For more control, use [`UpdateChecker`] directly. Pass `bypass_cache = true` to
//! always query crates.io, e.g. in CI.
//!
//! # Caching Behaviour
//!
//! Checks are cached in `{temp_dir}/updates_cache.json` for 1 hour and shared across
//! runs, so users are not spammed with update checks every time they run your tool.

mod core;

pub use core::{UpdateChecker, UpdateResult, check};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::standard_release;

    #[test]
    fn test_standard_release() {
        assert!(standard_release("1.0.0"));
        assert!(standard_release("2.4.1"));
        assert!(!standard_release("1.0.0-alpha"));
        assert!(!standard_release("2.4.1-rc1"));
        assert!(!standard_release("1.1.1-beta.1"));
    }

    #[test]
    fn test_basic_check() {
        let checker = UpdateChecker::new(true);
        let result = checker.check("reqwest", "0.13.0");
        assert!(result.is_some());
    }
}
