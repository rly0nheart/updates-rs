//! # updates
//!
//! A Rust library that checks for crate updates.
//!
//! **updates** only checks crates that are publicly listed on [crates.io](https://crates.io).
//!
//! # Quick Start
//!
//! Run the following command to add updates to your project's dependencies:
//!
//! ```shell
//! cargo add updates
//! ```
//!
//! # Usage
//!
//! ## Basic
//!
//! The easiest way to use this crate is with the [`update_check`] function:
//!
//! ```no_run
//!
//! fn main() {
//!     // Check for updates at startup
//!     updates::check(
//!         env!("CARGO_PKG_NAME"),
//!         env!("CARGO_PKG_VERSION"),
//!         false  // use cache
//!     );
//!     
//!     // Your application code here...
//!     println!("Hello, world!");
//! }
//! ```
//!
//! If an update is available, it will print to stderr:
//! ```text
//! Version 1.0.0 of my-tool is outdated. Version 1.2.0 was released 3 days ago.
//! ```
//!
//! ## Advanced
//!
//! For more control over the checking process, use [`UpdateChecker`] directly:
//!
//! ```no_run
//! use updates::UpdateChecker;
//!
//! fn main() {
//!     let checker = UpdateChecker::new(false);
//!     
//!     match checker.check("serde", "1.0.150") {
//!         Some(update) => {
//!             println!("Update available!");
//!             println!("Current version: {}", update.running_version);
//!             println!("Latest version: {}", update.available_version);
//!             
//!             if let Some(date) = update.release_date {
//!                 println!("Released: {}", date);
//!             }
//!         }
//!         None => {
//!             println!("You're on the latest version!");
//!         }
//!     }
//! }
//! ```
//!
//! ## Checking Multiple Crates
//!
//! ```no_run
//! use updates::UpdateChecker;
//!
//! fn check_dependencies() {
//!     let checker = UpdateChecker::new(false);
//!     
//!     let crates = vec![
//!         ("serde", "1.0.150"),
//!         ("tokio", "1.28.0"),
//!         ("regex", "1.8.0"),
//!     ];
//!     
//!     for (name, version) in crates {
//!         if let Some(update) = checker.check(name, version) {
//!             eprintln!("{}", update);
//!         }
//!     }
//! }
//! ```
//!
//! ## Bypassing the Cache
//!
//! If you need to always get the latest information (e.g., in a CI environment),
//! set `bypass_cache` to `true`:
//!
//! ```no_run
//! use updates::update_check;
//!
//! fn main() {
//!     // Always query crates.io, ignore cache
//!     update_check("my-tool", "1.0.0", true);
//! }
//! ```
//!
//! # Caching Behaviour
//!
//! Update checks are cached in your system's temp directory for 1 hour:
//!
//! - **Cache location**: `{temp_dir}/updates_cache.bin`
//! - **Cache duration**: 3600 seconds (1 hour)
//! - **Cache format**: Compact binary format using postcard serialisation
//!
//! The cache is automatically shared across multiple runs of your application,
//! so users won't be spammed with update checks every time they run your tool.
//!

mod core;

pub use core::{UpdateChecker, UpdateResult, update_check};

#[cfg(test)]
mod tests {
    use crate::core::{parse_version, standard_release};
    use super::*;

    #[test]
    fn test_standard_release() {
        assert!(standard_release("1.0.0"));
        assert!(standard_release("2.4.1"));
        assert!(!standard_release("1.0.0-alpha"));
        assert!(!standard_release("2.4.1-rc1"));
        assert!(!standard_release("1.1.1-beta.1"));
    }

    #[test]
    fn test_version_parsing() {
        assert!(parse_version("2.4.1") > parse_version("2.4.0"));
        assert!(parse_version("2.4.0") > parse_version("2.4.0-alpha"));
        assert!(parse_version("2.4.1") > parse_version("2.4.0"));
        assert!(parse_version("1.1.1") > parse_version("1.1.0"));
    }

    #[test]
    fn test_prerelease_ordering() {
        assert!(parse_version("1.0.0") > parse_version("1.0.0-rc1"));
        assert!(parse_version("1.0.0-rc2") > parse_version("1.0.0-rc1"));
        assert!(parse_version("1.0.0-beta") < parse_version("1.0.0-rc"));
        assert!(parse_version("1.0.0-alpha") < parse_version("1.0.0-beta"));
    }

    #[test]
    fn test_basic_check() {
        let checker = UpdateChecker::new(true);
        let result = checker.check("reqwest", "0.13.0");
        assert!(result.is_some());
    }
}
