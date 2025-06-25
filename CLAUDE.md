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

### Supported Languages (12+ total)

- Rust, C, C++, Java, JavaScript, TypeScript, Python, Ruby, Go
- YAML, HCL/Terraform, Makefile
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

1. Update version in `Cargo.toml`, `npm-package/package.json`, `pip-package/pyproject.toml`, and `pip-package/uncomment/__init__.py`
2. Commit and push changes
3. Create and push Git tag: `git tag v2.2.0 && git push origin v2.2.0`
4. Wait for Release workflow to build binaries
5. Create GitHub release manually to trigger Publish workflow
6. Packages automatically published to crates.io, npm, and PyPI

## AI Assistant Guidelines

When working on this project:

- Preserve the existing code style (Rust idioms, error handling patterns)
- Ensure new features maintain backward compatibility
- Add comprehensive tests for any new functionality
- The tree-sitter approach guarantees accuracy - focus on features, not edge cases
- Performance is acceptable - accuracy is more important than speed
- Use the visitor pattern when adding new analysis features

### Multi-Platform Distribution Learnings

The project supports distribution via:

- **Cargo**: Direct Rust installation
- **npm**: Package name is `uncomment-cli` (not `uncomment` which is taken)
- **PyPI**: Package name is `uncomment`

Key implementation details:

1. **Package Naming**: Always check npm/PyPI for name availability before choosing names

2. **Binary Distribution**:

   - npm uses custom `install.js` script to download platform-specific binaries
   - PyPI uses `uncomment.downloader` module with requests library
   - Both must handle HTTP redirects (GitHub releases redirect to S3)
   - Add `.npmignore` to exclude bin/ folder from npm package

3. **Version Format Differences**:

   - Cargo/npm use `2.1.1-rc.2` format
   - PyPI uses `2.1.1rc2` format (no hyphen/dot)
   - Publish workflow handles conversion automatically

4. **GitHub Actions Gotchas**:

   - Release workflow builds binaries using `taiki-e/upload-rust-binary-action`
   - Publish workflow only triggers on manual releases (not bot-created ones)
   - Uses dynamic versioning from Git tags - no hardcoded versions
   - npm version command fails if version already set - check first

5. **Critical Bug Fixes Applied**:
   - Use `WalkBuilder` from ignore crate for proper .gitignore handling
   - Implement HTTP redirect following in download scripts
   - Handle multi-byte UTF-8 characters in file processing

### Testing Distribution

When testing RC releases:

- Use sequential RC numbers (rc.1, rc.2, etc.)
- Delete failed releases/tags before retrying
- Watch workflow logs for specific errors
- Verify packages published: `npm view`, `pip index`, `cargo search`
- Test actual installation after publishing
