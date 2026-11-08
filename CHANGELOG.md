# CHANGELOG.md

# Changelog for Ghostty RPC

## [Unreleased]
- Initial development phase.
- Fix daemon silently wedging (process alive, presence frozen) after Discord quits or restarts: a watchdog now exits the process when the main loop stalls for 30s so launchd/systemd restarts it.
- README: launchd example now sets `ThrottleInterval` and log paths.
- Fix `refresh_interval` in the config file having no effect:
  - The config is now read from `~/.config/ghostty-rpc/config.toml` (or `$XDG_CONFIG_HOME`) on all platforms as documented; on macOS it was previously only read from `~/Library/Application Support/com.your_name.ghostty-rpc/`, which is still used as a fallback.
  - The documented `refresh_interval` key (seconds) is now honored; `refresh_interval_ms` is accepted as an override. Values are clamped to a 100ms minimum.
  - Config parse errors are now logged instead of silently falling back to defaults, and the loaded config path is logged at startup.
- Skip sending unchanged presence to Discord (re-sent every 15s as a keepalive) to stay under Discord's rate limit, so real changes appear sooner.

## [0.1.0] - 2023-10-01
### Added
- Basic structure of the project with modular files.
- CLI entry point in `src/main.rs` using `clap`.
- Discord Rich Presence integration in `src/discord.rs`.
- Monitoring of last executed commands and current working directory in `src/monitor.rs`.
- Configuration loading from TOML file in `src/config.rs`.
- Systemd service file for user-level service in `assets/ghostty-rpc.service`.
- Example configuration file in `assets/config.toml.example`.
- Installation and uninstallation scripts in `scripts/install.sh` and `scripts/uninstall.sh`.
- Shell hook example for capturing last executed command in `scripts/shell-hooks.sh`.
- CI/CD workflow for GitHub Actions in `.github/workflows/release.yml`.
- Documentation in `README.md` and versioning information.

## [0.1.1] - 2023-10-15
### Changed
- Improved error handling in `src/discord.rs`.
- Updated logging configuration in `src/main.rs`.

## [0.1.2] - 2023-10-30
### Fixed
- Resolved issues with reading the last command from `/tmp/ghostty_last_cmd`.
- Fixed bugs related to configuration loading in `src/config.rs`.

## [0.1.3] - 2023-11-15
### Added
- Support for excluding specific commands from being displayed in Discord presence.
- Enhanced logging for better debugging.

## [0.1.4] - 2023-11-30
### Changed
- Refactored code for better modularity and readability.
- Updated dependencies in `Cargo.toml`.

## [0.1.5] - 2023-12-15
### Added
- Implemented a feature to show/hide the current working directory based on configuration.
- Added unit tests for core functionalities.

## [0.2.0] - 2024-01-01
### Changed
- Major refactor of the project structure for improved maintainability.
- Updated documentation and examples in `README.md`.