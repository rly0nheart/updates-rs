use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
/// ```
/// use update_checker::UpdateResult;
///
/// let result = UpdateResult {
///     crate_name: "serde".to_string(),
///     running_version: "1.0.150".to_string(),
///     available_version: "1.0.200".to_string(),
///     release_date: None,
/// };
///
/// println!("{}", result);
/// // Version 1.0.150 of serde is outdated. Version 1.0.200 is available.
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

/// Response structure from the crates.io API.
#[derive(Deserialize)]
struct CratesIoResponse {
    #[serde(rename = "crate")]
    krate: CrateInfo,
    versions: Vec<VersionInfo>,
}

/// The crate-level summary crates.io returns, which already resolves "latest".
#[derive(Deserialize)]
struct CrateInfo {
    /// Highest non-prerelease, non-yanked version, if any
    max_stable_version: Option<String>,
    /// Most recently published version
    newest_version: String,
}

/// A single published version, used only to recover its release date.
#[derive(Deserialize)]
struct VersionInfo {
    num: String,
    created_at: String,
}

/// Main update checker with caching support.
///
/// # Examples
///
/// ```no_run
/// use update_checker::UpdateChecker;
///
/// let checker = UpdateChecker::new(false);
///
/// if let Some(result) = checker.check("serde", "1.0.150") {
///     println!("{}", result);
/// }
/// ```
pub struct UpdateChecker {
    /// Whether to bypass the cache on every check
    bypass_cache: bool,
    /// Check results, keyed by `name@version`
    cache: Mutex<HashMap<String, CacheEntry>>,
    /// Path to the persistent cache file
    cache_file: PathBuf,
}

impl UpdateChecker {
    /// Creates a new `UpdateChecker`, loading the on-disk cache if present.
    ///
    /// # Arguments
    ///
    /// * `bypass_cache` - If `true`, always queries crates.io instead of using cached
    ///   results. If `false`, uses cached results for up to 1 hour.
    ///
    /// # Examples
    ///
    /// ```
    /// use update_checker::UpdateChecker;
    ///
    /// let checker = UpdateChecker::new(false);
    /// ```
    pub fn new(bypass_cache: bool) -> Self {
        let cache_file = std::env::temp_dir().join("updates_cache.json");
        let cache = std::fs::read_to_string(&cache_file)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();

        UpdateChecker {
            bypass_cache,
            cache: Mutex::new(cache),
            cache_file,
        }
    }

    /// Writes the current cache to disk, ignoring any failure.
    fn save(&self) {
        if let Ok(cache) = self.cache.lock()
            && let Ok(data) = serde_json::to_string(&*cache) {
                let _ = std::fs::write(&self.cache_file, data);
            }
    }

    /// Checks if a newer version of a crate is available.
    ///
    /// Returns `None` if the crate is already up to date, or if the query fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use update_checker::UpdateChecker;
    ///
    /// let checker = UpdateChecker::new(false);
    ///
    /// if let Some(update) = checker.check("regex", "1.5.0") {
    ///     println!("Update available: {}", update.available_version);
    /// }
    /// ```
    pub fn check(&self, crate_name: &str, crate_version: &str) -> Option<UpdateResult> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs();

        let key = format!("{crate_name}@{crate_version}");

        if !self.bypass_cache
            && let Ok(cache) = self.cache.lock()
                && let Some(entry) = cache.get(&key)
                    && now.saturating_sub(entry.timestamp) < CACHE_EXPIRE_TIME {
                        return entry.result.clone();
                    }

        let include_prereleases = !standard_release(crate_version);
        let result = crates_io(crate_name, include_prereleases)
            .ok()
            .and_then(|(latest, created_at)| {
                let running = Version::parse(crate_version).ok()?;
                (running < Version::parse(&latest).ok()?).then(|| UpdateResult {
                    crate_name: crate_name.to_string(),
                    running_version: crate_version.to_string(),
                    available_version: latest,
                    release_date: created_at
                        .and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                        .map(|d| d.with_timezone(&Utc)),
                })
            });

        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(
                key,
                CacheEntry {
                    timestamp: now,
                    result: result.clone(),
                },
            );
        }

        self.save();
        result
    }
}

/// Queries crates.io for the latest version of a crate and its release date.
///
/// crates.io already resolves "latest" server-side (`max_stable_version` excludes
/// prereleases and yanked versions), so there is nothing to sort or filter here.
fn crates_io(
    package: &str,
    include_prereleases: bool,
) -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(3)))
        .build()
        .into();

    let data: CratesIoResponse = agent
        .get(&format!("https://crates.io/api/v1/crates/{package}"))
        .header(
            "User-Agent",
            &format!(
                "update-checker/{} (+{})",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_REPOSITORY")
            ),
        )
        .call()?
        .body_mut()
        .read_json()?;

    let latest = if include_prereleases {
        data.krate.newest_version
    } else {
        data.krate
            .max_stable_version
            .ok_or("no stable release found")?
    };

    let created_at = data
        .versions
        .iter()
        .find(|v| v.num == latest)
        .map(|v| v.created_at.clone());

    Ok((latest, created_at))
}

/// Returns `true` if `version` is a standard release rather than a prerelease.
pub(crate) fn standard_release(version: &str) -> bool {
    Version::parse(version).is_ok_and(|v| v.pre.is_empty())
}

/// Formats a datetime as a relative time string, or a full date past 7 days.
fn pretty_date(the_datetime: DateTime<Utc>) -> String {
    let diff = Utc::now().signed_duration_since(the_datetime);

    if diff.num_days() > 7 {
        return the_datetime.format("%x %X").to_string();
    }

    let (n, unit) = match diff {
        d if d.num_days() >= 1 => (d.num_days(), "day"),
        d if d.num_hours() >= 1 => (d.num_hours(), "hour"),
        d if d.num_minutes() >= 1 => (d.num_minutes(), "minute"),
        _ => return "just now".to_string(),
    };

    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// Checks for updates and prints to stderr if one is available.
///
/// The simplest way to add update checking to a CLI application.
///
/// # Examples
///
/// ```no_run
/// fn main() {
///     update_checker::check("my-cli-tool", env!("CARGO_PKG_VERSION"), false);
///
///     // ... rest of your application
/// }
/// ```
pub fn check(crate_name: &str, crate_version: &str, bypass_cache: bool) {
    let checker = UpdateChecker::new(bypass_cache);
    if let Some(result) = checker.check(crate_name, crate_version) {
        eprintln!("{result}");
    }
}

#[test]
fn cache_roundtrip() {
    let c = UpdateChecker::new(false);
    c.check("reqwest", "0.13.0");
    let raw = std::fs::read_to_string(&c.cache_file).expect("cache file written");
    assert!(raw.contains("reqwest@0.13.0"), "{raw}");
    // second checker must load it from disk
    assert!(UpdateChecker::new(false).cache.lock().unwrap().contains_key("reqwest@0.13.0"));
}
