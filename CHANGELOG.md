# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1](https://github.com/ilya-epifanov/llmwiki-tooling/compare/v0.3.0...v0.3.1) - 2026-07-23

### Added

- format links by repository style
- treat Markdown links as wiki edges

### Fixed

- scope page names to configured pages
- handle CommonMark destination edge cases
- preserve internal link semantics

### Other

- explain repository link styles

### Added

- first-class Obsidian and Markdown internal-link parsing
- explicit repository-configured link formatting with optional Reference-style links

### Fixed

- allow unmanaged path-addressed Markdown to share filename stems with managed pages

## [0.3.0](https://github.com/ilya-epifanov/llmwiki-tooling/compare/v0.2.0...v0.3.0) - 2026-07-05

### Added

- support repo-wide wiki scans

### Other

- describe wiki configuration model

## [0.2.0](https://github.com/ilya-epifanov/llmwiki-tooling/compare/v0.1.1...v0.2.0) - 2026-07-02

### Other

- Make wiki scan ignores configurable
- update ignore lockfile
- Fix relative root path handling
- Fix serde_yml API drift

## [0.1.1](https://github.com/ilya-epifanov/llmwiki-tooling/compare/v0.1.0...v0.1.1) - 2026-04-13

### Other

- update dependencies and bump rust-version to 1.87
- move cargo-dist config from dist.toml to Cargo.toml
