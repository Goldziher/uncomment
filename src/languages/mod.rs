pub mod config;
pub mod handlers;
pub mod registry;

pub use config::LanguageConfig;
pub use handlers::{LanguageHandler, get_handler};
pub use registry::LanguageRegistry;
