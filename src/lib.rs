// Uncomment - A tool for removing comments from code files
// Re-export public modules and types

pub mod cli;
pub mod language;
pub mod models;
pub mod processing;
pub mod utils;

// Re-export main types for convenience
pub use language::detection::detect_language;
pub use models::language::SupportedLanguage;
pub use models::line_segment::LineSegment;
pub use models::options::ProcessOptions;
pub use processing::file::process_file;
pub use utils::path::expand_paths;
