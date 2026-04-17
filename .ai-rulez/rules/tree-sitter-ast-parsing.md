---
priority: critical
---

# Tree-Sitter AST Parsing

All comment identification and removal MUST use tree-sitter AST parsing. Never use regex-based comment detection.

## Core Principles

- Use the AST visitor pattern in `src/ast/visitor.rs` for tree traversal
- Define `comment_nodes` for each language in `src/languages/registry.rs` or via configuration
- Cache parsed trees when processing multiple operations on the same file
- Handle tree-sitter query errors gracefully with descriptive error messages

## Implementation Guidelines

- When adding language support, identify the correct tree-sitter node types for comments (e.g., `comment`, `line_comment`, `block_comment`, `multiline_comment`)
- Use `tree-sitter parse <file>` to inspect AST structure and discover comment node names
- Ensure multi-byte UTF-8 content is handled correctly during byte-offset to character-offset conversion
- Tree-sitter parsing takes ~20-30ms per small file vs instant regex — always prioritize accuracy over speed

## Prohibited Patterns

- Never use regex to detect or match comments
- Never assume comment syntax is uniform across languages
- Never modify source code without first parsing the full AST
