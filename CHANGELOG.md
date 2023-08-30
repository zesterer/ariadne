# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

### Breaking changes

- Added missing `S: Span` bound for `Label::new` constructor.

- Previously labels with backwards spans could be constructed and
  only resulted in a panic when writing (or printing) the report.
  Now `Label::new` panics immediately when passed a backwards span.

### Added

### Removed

### Changed

### Fixed

# [0.3.0] - 2023-06-07

### Changed

- Upgraded concolor to `0.1`.
