//! Layered TOML configuration.
//!
//! Split four ways: `file` is the serde shape of a config file and how one layer merges into the
//! next, `manager` is [`ConfigManager`] — discovery, caching and per-file resolution — `exclude` is
//! the path filter every file collector applies, and `templates` is what `uncomment init` writes.

mod exclude;
mod file;
mod manager;
mod templates;

pub use exclude::ExcludeSet;
pub use file::{Config, GlobalConfig, LanguageConfig, PatternConfig, ResolvedConfig};
pub use manager::ConfigManager;
pub use templates::DetectionInfo;

use std::sync::Once;

/// The config file name `uncomment init` writes and discovery prefers.
pub const CONFIG_FILE_NAME: &str = ".uncomment.toml";

/// The published JSON Schema for a config file. `uncomment init` writes it as a `#:schema` directive
/// at the top of every generated config, so an editor that reads the directive — Taplo, and the
/// TOML extension built on it — validates and completes `.uncomment.toml` as it is written.
pub const SCHEMA_URL: &str = "https://raw.githubusercontent.com/Goldziher/uncomment/main/schema/uncomment.schema.json";

/// Every name discovery accepts, highest precedence first. The two after [`CONFIG_FILE_NAME`] are
/// deprecated: they are still read, so no existing config stops being honoured, and reading one
/// emits [`LEGACY_NAME_NOTICE`] once per run.
const CONFIG_FILE_NAMES: [&str; 3] = [CONFIG_FILE_NAME, ".uncommentrc.toml", "uncomment.toml"];

/// One deprecation notice per run, however many legacy-named configs the run reads. Process-global
/// rather than per-[`ConfigManager`] because the notice is about the user's files, not about a
/// particular manager, and the eager ancestor walk loads configs before any manager exists.
static LEGACY_NAME_NOTICE: Once = Once::new();
