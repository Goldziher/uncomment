/// Options for processing files
#[derive(Debug, Clone)]
pub struct ProcessOptions<'a> {
    /// Whether to remove TODO comments
    pub remove_todo: bool,
    /// Whether to remove FIXME comments
    pub remove_fixme: bool,
    /// Whether to remove documentation comments
    pub remove_doc: bool,
    /// Additional patterns to ignore
    pub ignore_patterns: &'a Option<Vec<String>>,
    /// Output directory for processed files
    pub output_dir: &'a Option<String>,
    /// Whether to disable default ignore patterns
    pub disable_default_ignores: bool,
    /// Whether to perform a dry run (don't write files)
    pub dry_run: bool,
}
