# CLAUDE.md - AI Assistant Context for Uncomment

## Project Overview

`uncomment` is a fast, accurate CLI tool written in Rust that intelligently removes comments from source code files using tree-sitter AST parsing. It was created to clean up AI-generated code that often contains excessive explanatory comments.

## Key Technical Details

### Architecture

- **Language**: Rust
- **CLI Framework**: clap v4.5
- **Parsing**: tree-sitter AST-based (v2.0.0+ complete rewrite)
- **Pattern Matching**: glob for file selection
- **Build System**: Cargo

### Core Functionality

The tool processes files to remove comments while preserving:

- Code structure and formatting
- Important metadata (pragmas, directives, attributes)
- TODO/FIXME comments (configurable)
- Documentation comments/docstrings (configurable)
- Comments marked with `~keep`
- Comments inside strings (100% accurate with AST)

### Supported Languages (9 total + partial JSON)

- Rust, C, C++, Java, JavaScript, TypeScript, Python, Ruby, Go
- Partial support: JSON/JSONC

### Testing & Quality

- Run tests: `cargo test`
- Build: `cargo build --release`
- Lint: `cargo clippy`
- Format: `cargo fmt`

### Important Implementation Notes

1. **Tree-sitter AST Parsing**: Uses tree-sitter parsers for accurate comment identification

2. **Comment Processing Pipeline**:

   - File → Language Detection → AST Parsing → Comment Node Identification → Removal → Output
   - AST visitor pattern traverses syntax tree to find comment nodes
   - 100% accurate - no false positives with strings or other constructs

3. **Edge Cases Handled**:

   - Comments within strings are automatically preserved (AST understands context)
   - No regex false positives
   - Language-specific comment types properly identified
   - Preservation rules applied based on comment content

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

1. Add tree-sitter parser dependency to `Cargo.toml`
2. Register language in `src/languages/registry.rs`
3. Define comment node types for the language's AST
4. Add preservation patterns in language config
5. Add test files

**Modifying comment preservation logic**:

- Core visitor logic is in `src/ast/visitor.rs`
- Preservation rules are in `src/rules/preservation.rs`
- Language configs are in `src/languages/config.rs`

### Performance Considerations

- Tree-sitter parsing is slightly slower than regex but 100% accurate
- Parser initialization is cached for efficiency
- Processes files in parallel when multiple paths provided
- Trade-off: ~20-30ms for small files vs instant regex (but with perfect accuracy)

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
- The tree-sitter approach guarantees accuracy - focus on features, not edge cases
- Performance is acceptable - accuracy is more important than speed
- Use the visitor pattern when adding new analysis features
