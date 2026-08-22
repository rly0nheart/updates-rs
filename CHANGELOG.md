# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.2.3] - 2026-08-22

### Changed

- Version comparison from a hand-rolled parser to _semver_
- Latest version now read from the crates.io API instead of sorted client-side

### Removed

- _regex_ and _libhuman_ dependencies, and chrono's default features

### Fixed

- On-disk cache never saved, as serde_json rejects non-string map keys

## [0.2.2] - 2026-07-15

### Changed

- human units dependency from _humanly_ to _libhuman_

## [0.2.1] - 2026-01-27

### Added

- CHANGELOG.md based on the [Keep a Changelog](https://keepachangelog.com/) format

### Changed

- Updated publish.yml workflow for automatic release notes based on CHANGELOG.md

## [0.2.0] - 2026-01-05

### Changed

- Dropped heavy dependencies for a lighter footprint

## [0.1.2] - 2026-01-01

### Added

- Publishing to crates.io
- Keywords to Cargo.toml for crates.io discoverability
- 3 second timeout for update checks

### Fixed

- Test failures

## [0.1.1] - 2026-01-01

### Added

- CI workflow with rust.yml
- Issue templates

### Fixed

- Minor fixes

## [0.1.0] - 2026-01-01

### Added

- Initial release
