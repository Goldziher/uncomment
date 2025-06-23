# CLAUDE.md - AI Assistant Context for Uncomment

## Project Overview

`uncomment` is a fast CLI tool written in Rust that intelligently removes comments from source code files. It was created to clean up AI-generated code that often contains excessive explanatory comments.

## Key Technical Details

### Architecture

- **Language**: Rust
- **CLI Framework**: clap v4.5
- **Pattern Matching**: glob + regex
- **Build System**: Cargo

### Core Functionality

The tool processes files to remove comments while preserving:

- Code structure and formatting
- Important metadata (pragmas, directives, attributes)
- TODO/FIXME comments (configurable)
- Documentation comments/docstrings (configurable)
- Comments marked with `~keep`
- Comments inside strings

### Supported Languages (10 total)

- Rust, C, C++, Java, JavaScript, TypeScript, Python, Ruby, Go, Swift

### Testing & Quality

- Run tests: `cargo test`
- Build: `cargo build --release`
- Lint: `cargo clippy`
- Format: `cargo fmt`

### Important Implementation Notes

1. **Smart Language Detection**: Uses file extensions to determine language (see `src/language/detection.rs`)

2. **Comment Processing Pipeline**:

   - File → Language Detection → Line Segmentation → Comment Removal → Output
   - Each line is segmented into Code/Comment parts
   - Preserves code structure by maintaining whitespace

3. **Edge Cases Handled**:

   - Comments within strings are preserved
   - Multi-line strings are tracked to avoid false positives
   - Nested comments are handled correctly
   - Test code comments can be preserved

4. **Git Integration**:
   - Respects .gitignore files by default
   - Can be used as a pre-commit hook

### Development Workflow

When modifying the tool:

1. Update relevant modules in `src/`
2. Add tests in the corresponding test modules
3. Run `cargo test` to ensure nothing breaks
4. Run `cargo clippy` for linting
5. Run `cargo fmt` for formatting
6. Update version in `Cargo.toml` if needed

### Common Modifications

**Adding a new language**:

1. Add language variant to `SupportedLanguage` enum in `src/models/language.rs`
2. Add file extensions in `src/language/detection.rs`
3. Define comment syntax in `src/language/definitions.rs`
4. Add default ignore patterns if needed
5. Add test files in `test-data/`

**Modifying comment preservation logic**:

- Core logic is in `src/processing/comment.rs`
- Default patterns are in `src/language/definitions.rs`

### Performance Considerations

- Uses efficient regex caching
- Processes files in parallel when multiple paths provided
- Minimal memory footprint through streaming processing

### Release Process

1. Update version in `Cargo.toml`
2. Run full test suite
3. Build release binary: `cargo build --release`
4. Binary location: `target/release/uncomment`

## AI Assistant Guidelines

When working on this project:

- Preserve the existing code style (Rust idioms, error handling patterns)
- Ensure new features maintain backward compatibility
- Add comprehensive tests for any new functionality
- Consider edge cases, especially around string handling and nested comments
- Performance is important - avoid unnecessary allocations or regex compilations
