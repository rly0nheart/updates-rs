use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Utc};
use regex::Regex;
use humanly::{HumanDuration, HumanTime};

/// Time in seconds before cache entries expire (1 hour).
const CACHE_EXPIRE_TIME: u64 = 3600;

/// A cached entry containing timestamp and optional update result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    /// Unix timestamp when this entry was cached
    timestamp: u64,
    /// The update result, if an update was available
    result: Option<UpdateResult>,
}

/// Information about an available crate update.
///
/// # Examples
///
/// ```no_run
/// use updates::UpdateResult;
/// use chrono::{DateTime, Utc};
///
/// // This is typically created by UpdateChecker, but you can construct it manually
/// let result = UpdateResult {
///     crate_name: "serde".to_string(),
///     running_version: "1.0.150".to_string(),
///     available_version: "1.0.200".to_string(),
///     release_date: None,
/// };
///
/// println!("{}", result);
/// // Output: Version 1.0.150 of serde is outdated. Version 1.0.200 is available.
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    /// Name of the crate
    pub crate_name: String,
    /// The version currently in use
    pub running_version: String,
    /// The latest available version
    pub available_version: String,
    /// When the latest version was released (if available)
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub release_date: Option<DateTime<Utc>>,
}

impl UpdateResult {
    /// Creates a new UpdateResult.
    ///
    /// # Arguments
    ///
    /// * `package` - The name of the crate
    /// * `running` - The current version string
    /// * `available` - The latest available version string
    /// * `release_date` - Optional RFC3339 timestamp of the release
    fn new(
        package: String,
        running: String,
        available: String,
        release_date: Option<String>,
    ) -> Self {
        let parsed_date = release_date.and_then(|d| {
            DateTime::parse_from_rfc3339(&d)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });

        UpdateResult {
            crate_name: package,
            running_version: running,
            available_version: available,
            release_date: parsed_date,
        }
    }
}

impl std::fmt::Display for UpdateResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Version {} of {} is outdated. Version {} ",
            self.running_version, self.crate_name, self.available_version
        )?;

        if let Some(date) = self.release_date {
            write!(f, "was released {}.", pretty_date(date))
        } else {
            write!(f, "is available.")
        }
    }
}

/// Response structure from crates.io API.
#[derive(Deserialize)]
struct CratesIoResponse {
    /// List of all versions for the crate
    versions: Vec<VersionInfo>,
}

/// Information about a specific crate version from crates.io.
#[derive(Deserialize)]
struct VersionInfo {
    /// Version number string (e.g., "1.0.0")
    num: String,
    /// RFC3339 timestamp of when this version was published
    created_at: String,
    /// Whether this version has been yanked
    yanked: bool,
}

/// Main update checker with caching support.
///
/// # Examples
///
/// ```no_run
/// use updates::UpdateChecker;
///
/// // Create a new checker with caching enabled
/// let checker = UpdateChecker::new(false);
///
/// // Check if serde needs an update
/// if let Some(result) = checker.check("serde", "1.0.150") {
///     println!("{}", result);
///     println!("Please update to: {}", result.available_version);
/// } else {
///     println!("You're up to date!");
/// }
/// ```
///
/// ```no_run
/// use updates::UpdateChecker;
///
/// // Create a checker that always queries crates.io (bypasses cache)
/// let checker = UpdateChecker::new(true);
///
/// match checker.check("tokio", "1.0.0") {
///     Some(update) => println!("Update available: {}", update.available_version),
///     None => println!("Already on latest version"),
/// }
/// ```
pub struct UpdateChecker {
    /// Whether to bypass the cache on every check
    bypass_cache: bool,
    /// In-memory cache of check results
    cache: std::sync::Mutex<HashMap<(String, String), CacheEntry>>,
    /// Path to the persistent cache file
    cache_file: Option<PathBuf>,
}

impl UpdateChecker {
    /// Creates a new UpdateChecker instance.
    ///
    /// # Arguments
    ///
    /// * `bypass_cache` - If `true`, always queries crates.io instead of using cached results.
    ///                    If `false`, uses cached results for up to 1 hour.
    ///
    /// # Examples
    ///
    /// ```
    /// use updates::UpdateChecker;
    ///
    /// // With caching (recommended for most use cases)
    /// let checker = UpdateChecker::new(false);
    ///
    /// // Without caching (always fetch fresh data)
    /// let checker_no_cache = UpdateChecker::new(true);
    /// ```
    pub fn new(bypass_cache: bool) -> Self {
        let cache_file = std::env::temp_dir()
            .join("updates_cache.bin");

        let mut checker = UpdateChecker {
            bypass_cache,
            cache: std::sync::Mutex::new(HashMap::new()),
            cache_file: Some(cache_file),
        };

        checker.load_from_permacache();
        checker
    }

    /// Loads cached data from disk into memory.
    fn load_from_permacache(&mut self) {
        if let Some(ref path) = self.cache_file {
            if let Ok(data) = fs::read(path) {
                if let Ok(cache) = postcard::from_bytes::<HashMap<(String, String), CacheEntry>>(&data) {
                    if let Ok(mut locked_cache) = self.cache.lock() {
                        *locked_cache = cache;
                    }
                }
            }
        }
    }

    /// Saves the current in-memory cache to disk.
    fn save_to_permacache(&self) {
        if let Some(ref path) = self.cache_file {
            if let Ok(locked_cache) = self.cache.lock() {
                if let Ok(data) = postcard::to_allocvec(&*locked_cache) {
                    let _ = fs::write(path, data);
                }
            }
        }
    }

    /// Checks if a newer version of a crate is available.
    ///
    /// # Arguments
    ///
    /// * `crate_name` - The name of the crate to check (e.g., "serde")
    /// * `crate_version` - The current version you're using (e.g., "1.0.150")
    ///
    /// # Returns
    ///
    /// * `Some(UpdateResult)` - If a newer version is available
    /// * `None` - If you're already on the latest version or if the query fails
    ///
    /// # Examples
    ///
    /// ```
    /// use updates::UpdateChecker;
    ///
    /// let checker = UpdateChecker::new(false);
    ///
    /// // Check a stable release
    /// if let Some(update) = checker.check("regex", "1.5.0") {
    ///     println!("Regex update available: {}", update.available_version);
    /// }
    ///
    /// // Check a prerelease (will also consider other prereleases)
    /// if let Some(update) = checker.check("tokio", "1.0.0-alpha.1") {
    ///     println!("Tokio prerelease update: {}", update.available_version);
    /// }
    /// ```
    pub fn check(&self, crate_name: &str, crate_version: &str) -> Option<UpdateResult> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let key = (crate_name.to_string(), crate_version.to_string());

        // Check cache
        if !self.bypass_cache {
            if let Ok(locked_cache) = self.cache.lock() {
                if let Some(entry) = locked_cache.get(&key) {
                    if now - entry.timestamp < CACHE_EXPIRE_TIME {
                        return entry.result.clone();
                    }
                }
            }
        }

        // Query crates.io
        let include_prereleases = !standard_release(crate_version);
        let result = match crates_io(crate_name, include_prereleases) {
            Ok(data) => {
                if parse_version(crate_version) >= parse_version(&data.version) {
                    None
                } else {
                    Some(UpdateResult::new(
                        crate_name.to_string(),
                        crate_version.to_string(),
                        data.version,
                        data.created_at,
                    ))
                }
            }
            Err(_) => None,
        };

        // Update cache
        if let Ok(mut locked_cache) = self.cache.lock() {
            locked_cache.insert(
                key,
                CacheEntry {
                    timestamp: now,
                    result: result.clone(),
                },
            );
        }

        self.save_to_permacache();
        result
    }
}

/// Data returned from a successful crates.io query.
struct CratesIoData {
    /// The version number
    version: String,
    /// When this version was created
    created_at: Option<String>,
}

/// Queries crates.io for the latest version of a crate.
///
/// # Arguments
///
/// * `package` - The crate name to query
/// * `include_prereleases` - Whether to include prerelease versions (alpha, beta, rc, etc.)
///
/// # Returns
///
/// * `Ok(CratesIoData)` - The latest version information
/// * `Err` - If the query fails or no suitable version is found
fn crates_io(package: &str, include_prereleases: bool) -> Result<CratesIoData, Box<dyn std::error::Error>> {
    let url = format!("https://crates.io/api/v1/crates/{}", package);
    let response = reqwest::blocking::Client::new()
        .get(&url)
        .header("User-Agent", "update-checker-rust/0.18.0")
        .timeout(Duration::from_secs(2))
        .send()?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()).into());
    }

    let data: CratesIoResponse = response.json()?;

    // Filter out yanked versions
    let mut versions: Vec<&VersionInfo> = data.versions
        .iter()
        .filter(|v| !v.yanked)
        .collect();

    if versions.is_empty() {
        return Err("No non-yanked versions found".into());
    }

    // Sort by version (newest first)
    versions.sort_by(|a, b| parse_version(&b.num).cmp(&parse_version(&a.num)));

    // Find the best version based on prerelease preference
    let version_info = versions
        .iter()
        .find(|v| include_prereleases || standard_release(&v.num))
        .ok_or("No suitable version found")?;

    Ok(CratesIoData {
        version: version_info.num.clone(),
        created_at: Some(version_info.created_at.clone()),
    })
}

/// Checks if a version string represents a standard release (not a prerelease).
///
/// A standard release contains only digits and dots (e.g., "1.0.0").
/// Prereleases contain additional identifiers (e.g., "1.0.0-alpha", "2.0.0-rc1").
///
/// # Arguments
///
/// * `version` - The version string to check
pub(crate) fn standard_release(version: &str) -> bool {
    version.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Formats a datetime as a human-readable relative time string.
///
/// # Arguments
///
/// * `the_datetime` - The datetime to format
///
/// # Returns
///
/// A human-readable string like "2 hours ago", "3 days ago", or a full date
/// if more than 7 days in the past.
fn pretty_date(the_datetime: DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(the_datetime);

    // If more than 7 days, show full date
    if diff.num_days() > 7 {
        return the_datetime.format("%x %X").to_string();
    }

    // If negative (future date), use HumanTime for future duration
    if diff.num_days() < 0 {
        let future_duration = Duration::from_secs(diff.num_seconds().abs() as u64);
        return format!("in {}", HumanTime::from(future_duration).to_string());
    }

    // For recent past dates, use HumanDuration
    let duration = Duration::from_secs(diff.num_seconds().max(0) as u64);
    let past_time = SystemTime::now() - duration;

    HumanDuration::from(Some(past_time)).to_string()
}

/// Convenience function that checks for updates and prints to stderr if one is available.
///
/// This is the simplest way to add update checking to your CLI application.
///
/// # Arguments
///
/// * `crate_name` - The name of your crate
/// * `crate_version` - The current version of your crate (typically from `env!("CARGO_PKG_VERSION")`)
/// * `bypass_cache` - Whether to bypass the cache and always query crates.io
///
/// # Examples
///
/// ```no_run
/// use updates::update_check;
///
/// fn main() {
///     // Check for updates at startup
///     update_check("my-cli-tool", env!("CARGO_PKG_VERSION"), false);
///
///     // ... rest of your application
/// }
/// ```
///
/// ```no_run
/// use updates::update_check;
///
/// // Force a fresh check (bypassing cache)
/// update_check("my-tool", "1.0.0", true);
/// ```
pub fn update_check(crate_name: &str, crate_version: &str, bypass_cache: bool) {
    let checker = UpdateChecker::new(bypass_cache);
    if let Some(result) = checker.check(crate_name, crate_version) {
        eprintln!("{}", result);
    }
}

/// Parses a version string into a comparable format.
///
/// This implements a version comparison algorithm similar to setuptools'
/// approach, handling standard versions, prereleases, and development versions.
///
/// # Arguments
///
/// * `s` - The version string to parse
///
/// # Returns
///
/// A vector of strings that can be compared lexicographically to determine
/// version ordering.
pub(crate) fn parse_version(s: &str) -> Vec<String> {
    let component_re = Regex::new(r"(\d+|[a-z]+|\.|-)").unwrap();
    let s_lower = s.to_lowercase();
    let mut parts = Vec::new();

    for part in component_re.find_iter(&s_lower) {
        let mut part_str = part.as_str().to_string();

        // Apply replacements to normalise prerelease identifiers
        part_str = match part_str.as_str() {
            "pre" => "c".to_string(),
            "preview" => "c".to_string(),
            "-" => "final-".to_string(),
            "rc" => "c".to_string(),
            "dev" => "@".to_string(),
            "alpha" => "a".to_string(),
            "beta" => "b".to_string(),
            _ => part_str,
        };

        if part_str.is_empty() || part_str == "." {
            continue;
        }

        if part_str.chars().next().unwrap().is_ascii_digit() {
            // Pad numbers for proper numerical comparison
            parts.push(format!("{:0>8}", part_str));
        } else {
            parts.push(format!("*{}", part_str));
        }
    }

    parts.push("*final".to_string());

    // Post-processing to clean up the parts
    let mut processed = Vec::new();
    for part in parts {
        if part.starts_with('*') {
            if part < "*final".to_string() {
                // Remove trailing "final-" markers before prerelease tags
                while processed.last() == Some(&"*final-".to_string()) {
                    processed.pop();
                }
            }
            // Remove trailing zeros
            while processed.last() == Some(&"00000000".to_string()) {
                processed.pop();
            }
        }
        processed.push(part);
    }

    processed
}
