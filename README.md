A Rust library that checks for crate updates.

> [!Note]
> **update-checker** only checks crate updates on [crates.io](https://crates.io).

## Quick Start

```shell
cargo add update-checker
```

## Usage

### Basic

The easiest way to use this crate is with the `check()` function:

```rust
fn main() {
    // Check for updates at startup
    update_checker::check(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        false, // use cache
    );

    // Your application code here...
    println!("Hello, world!");
}
```

If an update is available, it will print to stderr:

```text
Version 1.0.0 of my-tool is outdated. Version 1.2.0 was released 3 days ago.
```

### Advanced

For more control over the checking process, use `UpdateChecker` directly:

```rust
use update_checker::UpdateChecker;

fn main() {
    let checker = UpdateChecker::new(false);

    match checker.check("serde", "1.0.150") {
        Some(update) => {
            println!("Update available!");
            println!("Current version: {}", update.running_version);
            println!("Latest version: {}", update.available_version);

            if let Some(date) = update.release_date {
                println!("Released: {}", date);
            }
        }
        None => {
            println!("You're on the latest version!");
        }
    }
}
```

## Bypassing the Cache

If you need to always get the latest information (e.g., in a CI environment),
set `bypass_cache` to `true`:

```rust
fn main() {
    // Always query crates.io, ignore cache
    update_checker::check("my-tool", "1.0.0", true);
}
```

## Caching Behaviour

Update checks are cached in your system's temp directory for 1 hour:

- **Cache location**: `{temp_dir}/updates_cache.json`
- **Cache duration**: 3600 seconds (1 hour)
- **Cache key**: `{crate_name}@{version}`

The cache is automatically shared across multiple runs of your application,
so users won't be spammed with update checks every time they run your program.

## Special thanks
To the [update-checker](https://github.com/bboe/update_checker) Python package for inspiration
