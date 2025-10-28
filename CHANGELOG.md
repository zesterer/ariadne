# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Unreleased

### Breaking changes

### Added

### Removed

### Changed

### Fixed

# [0.6.0] - 2025-10-28

### Added

- Support for multiple help hints
- Support for multi-line help hints
- Implemented `Cache` for `&Source`

### Changed

- The Minimum Supported Rust Version (MSRV) is now 1.85.0
- Label ordering is now respected across multiple source files
- `ColorGenerator` is now usable in const contexts
- File references now have spaces, improving support for terminal emulator click-through

# [0.5.1] - 2025-03-13

### Added

- impl `Cache` for `&Source`
- Multiple and multiline help support

## Changed

- Use RPITIT instead of `Box<dyn ...>` for caches

# [0.5.0] - 2024-10-28

### Added

- Support for multi-line notes
- Support for `RangeInclusive` as spans

### Changed

- Made `Report::build` accept a proper span, avoiding much type annotation trouble

### Fixed

- Handling of empty lines
- `Config::new()` is now `const`
- Several subtle formatting bugs

# [0.4.1] - 2024-04-25

### Added

- Support for byte spans

- The ability to fetch the underlying `&str` of a `Source` using `source.as_ref()`

### Changed

- Upgraded `yansi` to `1.0`

# [0.4.0] - 2024-01-01

### Breaking changes

- Added missing `S: Span` bound for `Label::new` constructor.

- Previously labels with backwards spans could be constructed and
  only resulted in a panic when writing (or printing) the report.
  Now `Label::new` panics immediately when passed a backwards span.

### Added

- Support for alternative string-like types in `Source`

### Changed

- Memory & performance improvements

### Fixed

- Panic when provided with an empty input

- Invalid unicode characters for certain arrows

# [0.3.0] - 2023-06-07

### Changed

- Upgraded concolor to `0.1`.
