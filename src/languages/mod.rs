pub mod config;
pub mod handlers;
pub mod registry;

pub use config::LanguageConfig;
pub use handlers::{get_handler, LanguageHandler};
pub use registry::LanguageRegistry;
