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
- Custom preservation patterns (via configuration)
- Language-specific rules (via configuration)

### Supported Languages

**Built-in Languages** (14 total):

- Rust, C, C++, Java, JavaScript, TypeScript, Python, Go
- YAML, HCL/Terraform, Makefile, Shell/Bash
- JSON/JSONC (partial support)

**Extensible via Configuration**:

- Any language with a tree-sitter grammar can be added through configuration
- Examples: Swift, Kotlin, Vue, Svelte, Dart, Zig, Elixir, Julia, and many more

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

3. **Configuration System** (v2.4.0+):
   - TOML-based hierarchical configuration
   - Config discovery: CLI flag → `.uncommentrc.toml` (local) → `~/.config/uncomment/config.toml` (global) → defaults
   - Configuration merging with proper precedence
   - Pattern-based rules for file-specific overrides
   - Smart init command with language detection

4. **Dynamic Grammar Loading** (v2.4.0+):
   - **Git source**: Clones and compiles tree-sitter grammars from Git repositories
   - **Local source**: Loads grammars from local directories
   - **Library source**: Loads pre-compiled `.so`/`.dylib` files
   - **Caching**: Compiled grammars cached at `~/.cache/uncomment/grammars/`
   - **Grammar Manager**: Handles both static (built-in) and dynamic grammars

5. **Edge Cases Handled**:
   - Comments within strings are automatically preserved (AST understands context)
   - No regex false positives
   - Language-specific comment types properly identified
   - Preservation rules applied based on comment content

6. **Git Integration**:
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

**Adding a new language via configuration** (recommended):

1. Create or edit `.uncommentrc.toml`
2. Add language configuration with grammar source
3. Example:

   ```toml
   [languages.mylang]
   name = "My Language"
   extensions = ["ml"]
   comment_nodes = ["comment", "block_comment"]
   preserve_patterns = ["TODO", "FIXME"]

   [languages.mylang.grammar]
   source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-mylang", branch = "main" }
   ```

**Adding a built-in language** (for core languages):

1. Add tree-sitter parser dependency to `Cargo.toml`
2. Register language in `src/grammar/mod.rs` (static_languages HashMap)
3. Add language configuration in `src/languages/registry.rs`
4. Add test files in `tests/fixtures/`

**Modifying comment preservation logic**:

- Core visitor logic is in `src/ast/visitor.rs`
- Preservation rules are in `src/rules/preservation.rs`
- Language configs are in `src/languages/config.rs`

### Performance Considerations

- Tree-sitter parsing is slightly slower than regex but 100% accurate
- Parser initialization is cached for efficiency
- Processes files in parallel when multiple paths provided
- Trade-off: ~20-30ms for small files vs instant regex (but with perfect accuracy)
- Dynamic grammar loading: First-time Git grammar loading takes longer (clone + compile), but cached afterwards
- Grammar cache at `~/.cache/uncomment/grammars/` speeds up subsequent runs

### Release Process (v2.4.1+)

**Homebrew Release Pipeline** (Primary):

1. Update version in `Cargo.toml`, `npm-package/package.json`, `pip-package/pyproject.toml`, and `pip-package/uncomment/__init__.py`
2. Commit and push changes
3. Create and push Git tag: `git tag v2.4.1 && git push origin v2.4.1`
4. **Automated workflow** (`release-homebrew.yml`) handles:
   - Building binaries for all platforms
   - Creating GitHub releases with assets
   - **Automatically updating Homebrew formula** with correct URL and SHA256
5. Users can install via: `brew tap goldziher/tap && brew install uncomment`

**Legacy Package Distribution** (Temporarily Disabled):

- Cargo, npm, and PyPI distribution workflows disabled for Homebrew implementation
- Can be re-enabled by updating workflow triggers in `.github/workflows/`

### Key Modules (v2.4.0+)

**Configuration (`src/config.rs`)**:

- `Config` struct with global settings, language configs, and pattern rules
- Template generation methods: `template()`, `smart_template()`, `comprehensive_template()`
- TOML serialization/deserialization with serde

**Grammar Management (`src/grammar/`)**:

- `GrammarManager`: Central manager for all grammar loading
- `GitGrammarLoader`: Handles Git cloning and compilation
- Static language registration in `mod.rs`

**Init Command (`src/cli.rs`)**:

- `handle_init_command()`: Main init logic
- Language detection via file extension scanning
- Smart vs comprehensive template selection

## AI Assistant Guidelines

When working on this project:

- Preserve the existing code style (Rust idioms, error handling patterns)
- Ensure new features maintain backward compatibility
- Add comprehensive tests for any new functionality
- The tree-sitter approach guarantees accuracy - focus on features, not edge cases
- Performance is acceptable - accuracy is more important than speed
- Use the visitor pattern when adding new analysis features
- When adding grammars, prefer configuration over code changes
- Test dynamic grammar loading with actual tree-sitter repositories

### Multi-Platform Distribution Learnings

The project supports distribution via:

- **Homebrew**: Primary distribution method (v2.4.1+) - `brew tap goldziher/tap && brew install uncomment`
- **Cargo**: Direct Rust installation
- **npm**: Package name is `uncomment-cli` (not `uncomment` which is taken)
- **PyPI**: Package name is `uncomment`

Key implementation details:

1. **Homebrew Implementation** (v2.4.1+):
   - Uses git submodule (`homebrew-tap/`) to manage formula repository
   - Automated formula updates via `mislav/bump-homebrew-formula-action`
   - Source-based installation (builds from source with Rust dependency)
   - Workflow: `.github/workflows/release-homebrew.yml`
   - Formula location: `homebrew-tap/Formula/uncomment.rb`
   - Authentication: Uses `HOMEBREW_TOKEN` secret for cross-repo access

2. **Package Naming**: Always check npm/PyPI for name availability before choosing names

3. **Binary Distribution**:
   - npm uses custom `install.js` script to download platform-specific binaries
   - PyPI uses `uncomment.downloader` module with requests library
   - Both must handle HTTP redirects (GitHub releases redirect to S3)
   - Add `.npmignore` to exclude bin/ folder from npm package

4. **Version Format Differences**:
   - Cargo/npm use `2.1.1-rc.2` format
   - PyPI uses `2.1.1rc2` format (no hyphen/dot)
   - Publish workflow handles conversion automatically

5. **GitHub Actions Gotchas**:
   - Release workflow builds binaries using `taiki-e/upload-rust-binary-action`
   - Publish workflow only triggers on manual releases (not bot-created ones)
   - Uses dynamic versioning from Git tags - no hardcoded versions
   - npm version command fails if version already set - check first

6. **Critical Bug Fixes Applied**:
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
