pub mod cli;
pub mod language;
pub mod models;
pub mod processing;
pub mod utils;

pub use language::detection::detect_language;
pub use models::language::SupportedLanguage;
pub use models::line_segment::LineSegment;
pub use models::options::ProcessOptions;
pub use processing::file::process_file;
pub use utils::path::expand_paths;
