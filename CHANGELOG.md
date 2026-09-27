# Changelog

All notable changes to this project are documented in this file.

This changelog is generated from git tags and commit history.

## [Unreleased]

### Added

- Paths can be excluded outright: `exclude` under `[global]` in `.uncomment.toml`, and a repeatable
  `--exclude GLOB` on the command line that adds to it. A matching file is never collected, by any
  subcommand — the default run, `scan`, `keep` and `lint` alike — so a monorepo can keep
  `playground/`, generated trees and vendored dependencies out of every command without relying on
  `.gitignore`.

  Globs use the same dialect as `[patterns."<glob>"]` keys and are anchored the same way: relative to
  the directory of the config file that declared them, or to the invocation directory for
  `--exclude`. `exclude = ["vendor/**"]` also stops the walk descending into `vendor`, and a glob
  that does not compile fails the run when the config is read.

- Bazel and Java properties files are built-in languages. Starlark covers `BUILD`, `BUILD.bazel`,
  `WORKSPACE`, `WORKSPACE.bazel`, `WORKSPACE.bzlmod`, `MODULE.bazel`, `.bzl`, `.bazel` and `.star`;
  `properties` covers `.properties`, whose `#` and `!` comment forms the grammar reports alike.

- A language can claim whole filenames: `filenames` on `[languages.*]`, and `with_filenames` on the
  built-ins. It is the only way to reach a file with no extension, which is most of a Bazel
  repository. Matched case-sensitively — `BUILD` and `build` are different files to Bazel — and
  before any extension rule, so a claim on `BUILD.bazel` decides that file whatever else claims
  `.bazel`. An entry ending in `.*` claims by prefix, longest prefix first.

  This replaces the hardcoded `Makefile`/`Dockerfile`/`.bashrc` matching in language detection, which
  no configuration could extend: `Makefile`, `Dockerfile.*` and the shell rc names are now data on
  their own language, and a config-declared language gets the same reach. A section with neither
  `extensions` nor `filenames` is now rejected instead of registering a language nothing can match.

- Markdown is a built-in language, covering `.md`, `.markdown`, `.mdown` and `.mkd`. The grammar
  emits no comment node at all — an HTML comment is raw HTML to CommonMark, so `<!-- … -->` arrives
  as an `html_block`, the same node kind as a `<div align="center">` badge row or a `<details>`
  block. A block therefore counts as a comment only when its text is *nothing but* one comment:
  embedded HTML, a `<!DOCTYPE>`, an unterminated `<!--` (which CommonMark runs to the end of the
  document) and a comment sharing its line with anything else are all left alone.

  A comment inside a paragraph or a table cell is out of reach — it is inline content, not an
  `html_block` — and so is `.mdx`, whose `{/* … */}` form no grammar in the pack parses.

  Expressing this needed a new `LanguageHandler::classify_comment_node`, which can reject a node
  whose kind is configured as a comment or narrow the span that counts as one. It defaults to "no
  opinion", so every other language is unaffected.

- Go templates are a built-in language, covering `.tpl` — the Helm convention — plus Go's own `.tmpl`
  and `.gotmpl` and the HTML flavour `.gohtml`. A comment is only a comment inside an action, so
  `{{/* … */}}` is the whole syntax and a bare `/* … */` is literal output text; `keep` therefore
  writes `{{/* ~keep */}}`.

  Unlike `if_action`, `range_action` and `define_action`, which all span their own `{{`/`}}`, a comment
  action has no node covering it: the comment sits between two *bare sibling* delimiter tokens.
  Removing just the comment leaves `{{}}`, which the grammar rejects and Helm refuses to render
  (`missing value for command`). So the removal widens to the whole action whenever the comment's
  immediate siblings are the delimiters with nothing but whitespace between — a comment sharing its
  action keeps its own span, and the grammar places such a comment under an `ERROR` node where it has
  no delimiter siblings at all. Reported upstream as ngalaiko/tree-sitter-go-template#56.

  Two caveats. `{{ /* spaced */ }}` and `{{-/* x */-}}` produce no comment node, so they are left
  alone — a missed removal, never a corruption. And an inline comment keeps the whitespace around it
  (`hello {{/* x */}} world` → `hello  world`), because in a template that whitespace is rendered
  output and collapsing it would change what Helm emits.

  Widening needed `CommentInfo::widened`, the mirror of the narrowing `classify_comment_node`
  performs, and a `LanguageHandler::removal_span` hook that defaults to "the comment's own span" —
  so every other language is unaffected. Verified against 85 real Helm `.tpl` files: 48 modified, 256
  comments removed, zero empty actions, and a second pass is a no-op.

### Changed

- Word tags are matched without regard to case. `# todo`, `# Todo` and `# TODO` are now all
  preserved, as are `fixme`, `hack`, `xxx`, `bug`, `review`, `optimize`, `performance`,
  `security`, `deprecated`, `copyright`, `license` and `nosonar`. Previously only the
  all-uppercase spelling was, so a lowercase `// todo` was deleted — the spelling a tag is
  most often written in by hand.

  Pragmas stay case-sensitive, because the tools that read them are: `# noqa` is a directive
  ruff acts on and `# NOQA` is prose. `NOTE` and `WARNING` stay case-sensitive too — lowercase
  `note` and `warning` open ordinary prose sentences far more often than they label anything.

### Fixed

- `lint --fix` no longer rewrites English prose. Matching tags case-insensitively made the ordinary
  words `hack`, `todo` and `xxx` into tags wherever they appeared, so "this is a hack to work around
  the upstream bug" became "this is a TODO to work around the upstream bug". A miscased tag now has
  to open its comment — everything before it on the line must be delimiter or decoration — while a
  tag spelled exactly as configured is still a tag anywhere, because `TODO` in capitals is not a word
  anyone writes by accident. Measured on an 89k-file monorepo: 165 of 183 miscased tag words in code
  files were prose, and 167 comments would have been rewritten.

- bandit suppressions are no longer deleted. The comprehensive preservation set knew
  `bandit:` but not the `# nosec` form bandit actually reads, so a run silently
  re-enabled every security finding those comments had suppressed.

- markdownlint, Vale and doctoc directives are preserved. A markdown directive is an ordinary HTML
  comment, so nothing but the preservation set distinguishes `<!-- markdownlint-disable MD013 -->`
  from a removable aside — and deleting one turns a knowingly-suppressed lint error back into a
  build failure in a file nobody edited.

## [v3.8.0] - 2026-09-27

### Added

- `uncomment keep` writes `~keep` markers into the comments you chose to keep, so a
  decision taken over a whole repository can be applied in one pass instead of by hand.
  Select comments with `--from FILE` (a scan inventory with the lines you don't want
  deleted), `--id`, `--match SUBSTRING`, or `--all-removable`.

  A line comment gets `~keep` appended; a block, doc or docstring comment gets a plain
  marker line directly above it, at its own indentation. That split is not cosmetic — a
  Python docstring is a string node whose bytes are `__doc__` at runtime, and a marker
  written with a doc prefix (`///`, `##`) is classified as documentation and does nothing
  at all. Every marker is verified rather than assumed: the rewritten file is re-inspected
  under a configuration where a `~keep` marker is the only thing that can preserve a
  comment, and a marker that fails to protect its target is rolled back and the comment
  reported as unmarkable. Shebangs, language directives and comments sharing a line with
  code are refused for the same reason, never guessed at.

  Re-running is a no-op, including through a decisions file recorded before the first run:
  appending a marker changes a comment's bytes and therefore its id, so each comment
  answers to both its current id and the id it had before being marked.

- `uncomment scan` reports every comment a run would see — removed and preserved alike, each
  with the reason it is preserved — as JSONL (one record per line), JSON or text, and writes
  no source file. It honours the same selection flags as a real run, so `--only removable`
  reproduces exactly what that run would delete. Each record carries an id derived from the
  file path, the comment's own bytes and its occurrence index, deliberately excluding line and
  byte offsets, so editing code elsewhere in the file does not invalidate it; `uncomment keep
  --from` reads those ids back. Ids are truncated, so a collision is possible rather than
  impossible: colliding ids are widened to the full digest and reported, and every id in a
  report is unique.

  `--group-identical` collapses comments whose normalized text is identical into one record
  with a site list, which is what makes a large repository decidable — one judgement instead
  of hundreds. Output is byte-identical regardless of `-j`.

- `uncomment lint` checks tag comments — TODO, FIXME, HACK, XXX — and removes nothing, so
  it can run as its own pre-commit hook or CI step. Three rules, each `error`, `warn` or
  `off`: the tag is the canonical one (`FIXME` → `TODO`), it carries a tracking key
  (`TODO(PROJ-123):`), and that key is not the issue the current branch is working on —
  that one closes when the branch merges, which would leave the TODO pointing at a dead
  ticket and the work invisible. `--fix` rewrites non-canonical tags in place; a missing key
  is reported rather than invented, unless `--todo-key KEY` says which one to write.

  Every part of the convention is configuration, under `[lint]` in `.uncomment.toml` —
  the tags, the canonical one, the key pattern, and how the current issue is read out of
  the branch name — because no two repositories agree on it. Linting is off until
  `enabled = true` appears under `[lint]`, and a run that matches files under no such
  config says so rather than reporting a clean bill of health.

  Tags are read from parsed comments, so a `TODO` inside a string
  literal is not a violation. Adoption on an existing codebase is what `--changed-only`
  (lint only what differs from the base ref) and `--write-baseline` / `--baseline` (record
  today's violations, report them without failing) are for; because comment ids exclude
  line numbers, a baseline survives unrelated edits to the files it covers.

- `Processor::inspect` reports **every** comment in a file, kept and removed alike, each
  with the reason it was kept — a matched pattern, documentation, a file header, a
  shebang, its own `~keep` marker, a neighbour's marker, or a language directive.
  `plan_removals` is now a filter over it and keeps its previous signature and output.
  This is the groundwork for the forthcoming `scan`, `keep` and `lint` subcommands; the
  CLI's behaviour is unchanged, verified byte-for-byte against the previous release
  binary over a 714-file tree.

- Library modules `edit` (validated multi-edit application, rejecting overlapping,
  out-of-bounds and non-character-boundary ranges rather than corrupting a file), `git`
  (current branch and its issue key, read from `.git/HEAD` including the linked-worktree
  `gitdir:` indirection), `scan::id` (comment identifiers that deliberately exclude line
  and byte offsets, so they survive unrelated edits to the file) and `paths` (lexical
  normalization, so a `..` component can no longer defeat a containment check).

### Changed

- `uncomment lint` now matches tags regardless of casing, where before only the literal casing in
  `lint.tags` was a tag at all. This is a behaviour change: a tree that lints clean today can start
  reporting violations, because a miscased tag is a `tag-not-canonical` violation and `--fix`
  rewrites it to `canonical_tag` — `# todo: x` becomes `# TODO: x`, and `# fixme: x` becomes
  `# TODO: x`. The rule now covers two defects rather than one, a tag spelled differently from the
  canonical tag and the canonical tag spelled in the wrong casing, and the message says which:
  "`TODO` should be written as `TODO`" would say nothing. Set `case_sensitive_tags = true` under
  `[lint]` for exactly the previous behaviour; it layers key by key like every other key in that
  table, so one subdirectory can opt out without restating the rest.

  The key half of the convention did not loosen along with the tag half. `key_pattern` is a regex
  you supply and it spells the tags out itself, so a miscased tag is reconciled before the pattern
  runs rather than the pattern being compiled case-insensitively, which would have accepted a
  miscased key too: `# todo(AMVP-1):` counts as keyed, while `# TODO(amvp-1):` and
  `# todo(amvp-1):` do not. Two entries in `lint.tags` that differ only in casing are now a
  configuration error, because case-insensitive matching makes them one tag and which of the two
  `--fix` should write is unanswerable.

- The configuration file a project is read from is now `.uncomment.toml`, and that is the name
  `uncomment init` writes when no `--output` is given. One name, spelled the way the tool is,
  instead of an `rc` suffix borrowed from a different ecosystem and a second spelling beside it.
  Both previous names are still discovered — `.uncommentrc.toml` first, then `uncomment.toml` —
  so no existing configuration stops being read; they are deprecated, and the first config a run
  loads from either name prints one notice on stderr naming that file and the name to move to.
  Nothing is renamed for you and nothing fails. A directory holding more than one of the three
  uses the highest-precedence name outright rather than merging them, which is the rule the older
  pair already followed.

- The library's `keep::Unmarkable::NoLineCommentToken` is now `NoMarkerToken`, since a
  missing line-comment token is no longer on its own a reason a comment cannot be marked.

- Unknown keys in a configuration file are now a load error naming the
  offending key, instead of being silently ignored. A typo such as `remove_todoz = true`
  previously parsed fine and did nothing. Every shipped `uncomment init` template still
  round-trips, and every key documented in the README remains valid.

- Config precedence is now unambiguous in two cases that previously depended on sort
  order: a directory holding more than one of the accepted config file names uses the
  highest-precedence one outright, and the user-level global config is always the
  lowest-precedence layer. A config between the invocation directory and the git root
  now applies at all — previously a repo-root config was invisible from a subdirectory.

- `--help` no longer lists `clippy::` among the directives preserved by default. It is
  only in the comprehensive rule set, so the claim was misleading.

- The minimum supported Rust version is now declared: `rust-version = "1.90"`. Edition 2024
  alone needs 1.85, but `tree-sitter` 0.27 and `ordered-float` 5.5 both require 1.90, so
  that was already the real floor — it is now stated and enforced in CI.

- The published crate contains only `src/`, `Cargo.toml`, `README.md`, `LICENSE` and
  `CHANGELOG.md`. The repository root also holds the npm and pip wrappers, the test corpus
  and the AI-assistant configuration, none of which a crates.io consumer can use.

- The npm package's `tar` dependency moved from `^6` to `^7`, which is where every published
  fix for the outstanding node-tar advisories lives. The extraction call site is unchanged
  across the major, and a full install — download, checksum verification, extract, run — was
  exercised against 7.5.22. `adm-zip` moved to `^0.6.1` alongside it; its one behaviour change
  on the `0.6` line concerns directory entries, and the published Windows archive holds a
  single flat file, so the Windows install path was verified to extract byte-for-byte what
  `unzip` produces.

- The pip package requires Python 3.10 or newer, and the npm package Node 22 or newer. Both
  previous floors — Python 3.8 and Node 18 — are past end of life, and the Python one was no
  longer buildable anyway: the current `setuptools` needs 3.10. Neither wrapper does more than
  download and exec the binary for your platform, so the dropped versions were not being
  exercised by anything. Releases are now built on Python 3.14 and Node 24.

- The pip package declares its licence as a PEP 639 SPDX expression (`license = "MIT"`) rather
  than a table plus a classifier, both of which setuptools deprecates with a stated removal
  date. `certifi` moved to `>=2026.7.22`, and the build requirement to `setuptools>=84`.

### Fixed

- A `[lint]` section in a config file no longer breaks every other subcommand. Rejecting
  unknown keys made `[lint]` a hard parse error — `unknown field 'lint', expected one of
  'global', 'languages', 'patterns'` — for `uncomment`, `scan` and `keep` alike; only
  `lint` itself worked, because it stripped the table before deserializing. Enabling
  linting therefore disabled everything else in that tree. One config file now serves both.

  A nested `[lint]` table also amends the table above it key by key rather than replacing
  it outright, so naming one rule's severity no longer resets `enabled` or the tag
  vocabulary — the same layering `[global]` already had. A config rejected below the
  invocation directory fails the lint run before `--fix` rewrites anything.

- A comment in a language with no line-comment form can now be marked. `uncomment keep`
  refused every block comment in CSS, HTML and the other block-only languages for want of
  a token to write the marker line with — 108 of 3261 comments in one real run. A block
  comment alone on its line satisfies the marker guard exactly as a line comment does, so
  the marker falls back to `<!-- ~keep -->` / `/* ~keep */`.

  Recognising that form back is what keeps a decisions file idempotent. An unrecognised
  `/* ~keep */` is reconstructed with its marker stripped as `/* */`, which claims the id
  of any real empty comment in the file — and `keep` then refuses the whole run as
  ambiguous rather than marking what was asked for. Writing and recognising now share one
  list of candidate marker lines, so the two cannot drift apart.

- Ruby and Perl no longer offer a block-comment pair that cannot carry a marker. `=begin`
  /`=end` and POD are recognised only at column 0, so they cannot wrap an indented marker
  line; both languages now declare their line form only.

- The library's `paths::is_ancestor_of` and `paths::repo_relative` no longer treat a root that
  names the current directory as containing everything. `.`, `""` and `a/..` all normalize to
  the empty path, which `Path::starts_with` calls a prefix of every path — so `/etc/passwd` and
  `../sibling` read as contained, and `repo_relative` handed an absolute path straight back as
  though it were already relative. Every command resolves its root to an absolute path first, so
  this was latent rather than live; it is fixed because the ids `scan` and `keep` derive from that
  string would otherwise differ between checkouts.

- `uncomment keep` now reports a `[languages.*]` section it could not register, as the
  default run, `scan` and `lint` already did. A section naming a language no grammar exists
  for registers nothing — not even its extensions, so the files it was written for are never
  collected — and `keep` answered `0 marked, 0 already marked, 0 unmarkable, 0 unresolved`,
  which reads as "nothing needed marking".

- A custom language declared in `[languages]` is now recognized while files are being
  collected, so a file whose extension exists only in your config is processed instead of
  counted as unsupported and skipped. Collection was built from the built-in languages
  alone and the config was consulted only afterwards, per file — so overriding an
  extension that was already built in worked, and declaring a genuinely new one silently
  did nothing. Declarations in a config file *below* the invocation directory, under the
  paths being processed, now count too: deepest declaration wins.

- `npm install -g uncomment-cli` no longer installs uncomment 2.0.0. A 15 MB macOS arm64
  binary from that release was committed at `npm-package/bin/uncomment` and shipped
  verbatim in every tarball since, so the postinstall download was overwritten on macOS
  and the wrong binary ran on every other platform. `bin/uncomment` is now a launcher that
  execs the binary the postinstall step fetched for your platform, and reports a clear
  error if that step did not run.

- `[patterns."<glob>"]` config sections now actually do something. They were parsed and
  then never consulted — resolution only ever read `[global]` — so every per-path
  override emitted by `uncomment init` silently did nothing. Globs are relative to the
  directory of the config file that declared them, are matched per file (so
  `**/*.spec.ts` works), and are applied after `[global]` and before `[languages]`
  overrides. Overlapping globs resolve by specificity — deeper pattern wins — under a
  total order fixed at config load, so results no longer depend on hash iteration order.

  **This changes behaviour for anyone who ran `uncomment init` and kept the generated
  `[patterns]` examples**, which set `remove_todos = true` under `tests/**/*` and
  `remove_docs = true` for `**/*.spec.*` and `**/*.generated.*`.

- `XXX` and `NOSONAR` comments are now preserved by default, as `--help` has always
  claimed. Neither pattern was in the default rule set, so `// XXX: load-bearing` was
  silently deleted by a plain `uncomment` run despite being documented as protected.

- A config file that does not contain a `[global]` section no longer resets every global
  setting to its default. A nested `.uncomment.toml` carrying only `[patterns]` used to
  deserialize as all-defaults and silently erase the enclosing config's `remove_docs`,
  `remove_todos` and the rest; only the keys a file actually contains now override the
  layer above it.

- `preserve_patterns = []` inside a `[patterns."<glob>"]` section now clears the inherited
  patterns instead of being ignored. Omitting the key inherits, an empty list clears, and
  a non-empty list extends — previously there was no way to opt a path back out.

- A config file that fails to parse or validate is now a hard error naming the file,
  instead of being silently replaced by defaults. The previous behaviour was the dangerous
  one: a typo in `.uncomment.toml` meant the run continued under default settings and
  deleted comments the config existed to protect. A config discovered below the invocation
  directory is also recorded and reported, and fails the run's exit code.

- A `..` component in an input path can no longer reach a sibling directory's config or
  escape the git-root ceiling. `uncomment ../other` used to resolve config against the
  literal path, so `repo/sub/..` matched `repo/sub`'s settings — `Path::starts_with` and
  `parent` are both lexical. Paths are normalized before any containment check, which also
  removes a spurious "`[languages]` … is ignored" warning on such a path.

## [v3.7.0] - 2026-09-18

### Added

- Built-in Objective-C support. `.m` files are now recognized out of the box and
  routed through the C-family handler with the `objc` grammar, which models
  `//` and `/* … */` comments. Because `.m` is also the MATLAB extension,
  `uncomment` treats it as Objective-C by default — in a mixed project pass only
  the Objective-C paths. Headers (`.h`) keep the C configuration, and
  Objective-C++ (`.mm`) is not part of built-in support
  ([#124](https://github.com/Goldziher/uncomment/issues/124)).

### Changed

- Bumped dependencies: `tree-sitter` 0.27.0, `tree-sitter-language-pack` 1.20.0,
  `dirs` 7.0.0, and `toml` 1.1.6.

## [v3.6.0] - 2026-08-30

### Added

- `~keep` on its own line now protects the comment directly beneath it. The marker is
  plain comment text, so one written inside a doc comment is republished by everything
  that consumes doc comments — rustdoc, `utoipa`-generated OpenAPI schemas, generated
  API clients, editor hover text. A marker on a preceding `//` line protects the comment
  without rendering anywhere. It extends across a contiguous run of comments; a blank
  line or any code between comments ends the run.

### Changed

- A redundant `~keep` inside a doc comment is now stripped from the doc text and
  reported (`stripped 2 redundant ~keep markers`). The comment itself is untouched.
  Such a marker does nothing where it sits — doc comments are preserved anyway unless
  `--remove-doc` is set — while the token travels outward into rendered documentation.
  The marker is left in place when `--remove-doc` is set, where it is the only thing
  protecting that doc comment, and prose *about* the marker is never rewritten: a line
  containing backticks, or a commented-out code sample, reads as documentation rather
  than as a directive.
- Per-file output no longer leads with a removal count when nothing was removed, so a
  run that only stripped markers reports just that.
- Bumped dependencies: `tree-sitter-language-pack` 1.15.12, `tree-sitter` 0.26.13,
  `saphyr` 0.0.12, and `ignore` 0.4.33.

### Fixed

- `uncomment`'s own `--help` no longer prints `~keep` in its option descriptions
  ([#113](https://github.com/Goldziher/uncomment/issues/113)).
- Removed-comment counts and line lists no longer double-count comments that the
  grammar records as nested nodes. A Rust `///` line arrives twice, as the outer
  `line_comment` and as the inner doc node, so `--remove-doc` over two doc comments
  reported `would remove 4 (L1–2, L1–2, L4–5, L4–5)` instead of
  `would remove 2 (L1–2, L4–5)`. Only the reporting was affected; the removal itself
  already collapsed the overlap.

## [v3.5.0] - 2026-07-20

### Added

- Block-level `~keep`. A single `~keep` now preserves an entire contiguous block of
  standalone single-line comments, not just the marked line. Multi-line rationale
  comments (which tree-sitter models as one node per line) no longer need `~keep` on
  every line. Scope is deliberately narrow: only `~keep` extends across a block; other
  preservation rules stay per-comment; trailing comments never anchor or join a block;
  block comments are already single nodes; and a blank line or intervening code ends
  the block.

### Changed

- Bumped dependencies: `tree-sitter-language-pack` 1.13.2, `tree-sitter` 0.26.11,
  `toml`, `saphyr`, `ignore`, and `anyhow`.

## [v3.4.0] - 2026-07-12

### Added

- Per-file removal locations in the output. Each modified file now lists the line
  numbers and ranges of the comments removed (e.g. `removed 4 (L3, L7–9, L15)`); the
  list is capped with a `+N more` suffix on large files, and `--verbose` expands it to
  a per-comment preview.
- A preservation hint printed once at the end of a run when comments were removed,
  explaining how to keep them: the `~keep` marker, the TODO/FIXME/docstring defaults,
  `--ignore "<pattern>"`, `preserve_patterns`, and `--dry-run --diff`.
- `--quiet` / `-q` to suppress per-file output for scripting; the summary and errors
  still print, and files are still written.

### Changed

- `--diff` now works on real runs, not only in combination with `--dry-run`.

### Fixed

- `--diff` output no longer misaligns. It previously compared original and processed
  files line-by-line by index, so every line after the first removed one was
  mislabeled; it now renders an exact diff from the removed byte ranges with
  surrounding context.

## [v3.3.0] - 2026-07-10

### Added

- Colorized, terminal-aware CLI output. Colors render on a TTY and degrade to clean
  plain text automatically when piped or when `NO_COLOR` is set. Styled help groups
  options into Comment selection / File selection / Output / Performance, adds value
  names and usage examples, and colors the summary, diff, and error output.
- Progress bar for large, non-verbose runs (shown only on a terminal for 20+ files).
- Brand identity: a logo (mark, wordmark, and hero banner as SVG + PNG) and a
  restructured README with a hero, value proposition, and badges.

### Changed

- Upgraded dependencies: `anstream` to 1.0, `ignore` to 0.4.28, `indicatif` to 0.18
  (dropping the unmaintained `number_prefix`, RUSTSEC-2025-0119).

## [v3.2.0] - 2026-07-09

### Added

- In-memory `plan_removals` API for computing comment removals without writing files.

## [v3.1.0] - 2026-07-09

### Fixed

- macOS x86-64 (Intel) support ([#87]). The `x86_64-apple-darwin` binary is now built
  natively on the macOS 15 Intel runner, and tree-sitter parsers are downloaded at
  runtime (tree-sitter-language-pack 1.12.5 ships the `macos-x86_64` parser bundle).

### Changed

- Grammars are now downloaded on demand at runtime (tree-sitter-language-pack dynamic
  mode) instead of being statically linked into the binary. This yields a much smaller
  binary and makes every tree-sitter-language-pack language available. Parsers are cached
  under `~/.cache/tree-sitter-language-pack`; the first run on a platform downloads a
  one-time (~17 MB) parser bundle and requires network access, and is offline thereafter.
- Migrated the release pipeline from goreleaser to a native per-platform GitHub Actions
  build matrix with a draft-then-finalize flow (Linux built in manylinux_2_28 containers
  for a glibc 2.28 floor).

[#87]: https://github.com/Goldziher/uncomment/issues/87

## [v3.0.3] - 2026-05-21

### Changed

- Updated Rust dependencies, including tree-sitter-language-pack and tree-sitter.
- Normalized pre-commit hook configuration to use shared kreuzberg-dev hooks.

## [v3.0.1] - 2026-04-17

### Changed

- Homebrew formula now installs pre-built binaries via goreleaser instead of compiling from source.

## [v3.0.0] - 2026-04-17

### Breaking Changes

- Replaced 31 individual tree-sitter grammar dependencies with [tree-sitter-language-pack](https://github.com/kreuzberg-dev/tree-sitter-language-pack).
- Removed `GrammarSource`, `GrammarConfig`, and `[languages.*.grammar]` TOML config sections.
- Removed `src/grammar/` module (`GrammarManager`, `GitGrammarLoader`).

### Added

- 18 new built-in languages (49 total): Dockerfile, Scala, Dart, R, Julia, Zig, Clojure, Elm, Erlang, Vue, Svelte, SCSS, LaTeX, Fish, Perl, Groovy, OCaml, Fortran.
- 306 languages available via tree-sitter-language-pack with automatic grammar downloading.
- Dockerfile special filename detection (Dockerfile, Dockerfile.\*).
- Comprehensive test fixtures for all 49 built-in languages.
- Edge-case tests for UTF-8, empty files, and trailing comments.

### Performance

- Replaced HashMap/HashSet with ahash (AHashMap/AHashSet).
- Rewrote comment removal as single-pass forward copy with memchr SIMD byte searching.
- Eliminated mutex contention from rayon parallel loop.
- Removed per-comment String allocations from CommentInfo.
- Zero-allocation preservation rule patterns for built-in rules.

### Fixed

- Python docstring detection for tree-sitter-language-pack grammar.
- rayon thread pool creation panic (uses error propagation instead of unwrap).
- C# tslp name (`c_sharp`) for crates.io compatibility.
- Silent test failures in linting directive tests.

### Documentation

- Updated all READMEs for tree-sitter-language-pack.
- Rewrote example config files.

## [v2.11.0] - 2026-02-16

### Added

- Built-in language support for Haskell, HTML, CSS, XML, SQL, Kotlin, Swift, Lua, Nix, PowerShell, Protocol Buffers, and INI.
- Language fixtures and coverage for the new built-in grammars.

### Changed

- Updated tree-sitter dependencies and static grammar registrations for expanded built-in language support.
- Expanded language registry mappings and smart-init extension detection for newly supported languages.
- Updated tests to reflect built-in grammar behavior for languages that no longer require dynamic grammar configuration.

### Documentation

- Updated README language coverage and dynamic grammar examples to reflect the current built-in language set.

## [v2.10.4] - 2026-01-08

### Changed

- Updated Rust dependencies.

## [v2.10.3] - 2025-12-13

### Changed

- Switched crates.io publishing in CI to Trusted Publishing (OIDC), removing the need for a long-lived `CARGO_TOKEN`.
- Windows release assets now target `x86_64-pc-windows-gnu` only (32-bit Windows is no longer supported by the binary wrappers).

## [v2.10.2] - 2025-12-13

### Changed

- Aligned publishing workflows with `gitfluff`: GoReleaser-built GitHub release assets plus split registry publishing jobs (crates.io, npm, PyPI).
- Publishing is now idempotent: if a version is already published, CI skips re-publishing instead of failing.
- Windows release assets are now published as `.zip` archives (Linux/macOS remain `.tar.gz`).

### Fixed

- Python wrapper now downloads binaries without the `requests` dependency and caches per-version; use `UNCOMMENT_BINARY` to override the binary path.

## [v2.10.1] - 2025-12-13

### Fixed

- Preserve shebang lines (e.g. `#!/usr/bin/env bash`) even when not on the first line, and even when `--no-default-ignores` is used
- Avoid broken pipe panics when piping output (e.g. `uncomment ... | head`)
- Preserve common auto-generated / do-not-edit file header comments
- Preserve C/C++ preprocessor trailing comments (e.g. `#endif /* HEADER_GUARD */`)

### Changed

- Summarize unsupported files once (instead of printing per-file errors), with examples in `--verbose`
- Make dry-run output quiet by default; add `--diff` to show line-by-line diffs
- Detect and warn about potentially important comment removals (with examples in `--verbose`)

## [v2.10.0] - 2025-12-12

### Added

- Built-in language support: Ruby, PHP, Elixir, TOML, C#

### Fixed

- Preserve Go embed/cgo directives and cgo preambles
- Preserve Ruby magic comments and YARD docs by default

### Changed

- Supported file detection now follows the language registry

### Documentation

- Added `CHANGELOG.md`

## [v2.9.2] - 2025-12-01

### Chore

- chore: bump version to 2.9.2 (b559f01)

## [v2.9.1] - 2025-12-01

### Chore

- chore: bump version to 2.9.1 (d0aec0b)
- chore: updated deps (c85ceb3)

### Other

- ci(deps)(deps): bump actions/checkout from 5 to 6 (b6ca642)
- build(deps)(deps): bump clap in the production-dependencies group (fb0565f)
- ci(deps)(deps): bump actions/setup-node from 5 to 6 (3020595)

## [v2.9.0] - 2025-11-12

### Chore

- chore: migrate to saphyr and update dependencies (0090245)
- chore: align versions to 2.9.0 and clean clippy (b1989fe)

### Other

- ci(deps)(deps): bump actions/download-artifact from 5 to 6 (dad93d1)
- ci(deps)(deps): bump actions/upload-artifact from 4 to 5 (f6ea7a6)

## [v2.8.3] - 2025-10-11

### Fixed

- fix: preserve rust attribute macros when removing doc comments (279d212)

### Chore

- chore: run prek hooks (33c8744)
- chore: bump version to 2.8.3 (ab84add)
- chore: strip redundant comments (d2dcdcf)

## [v2.8.2] - 2025-10-11

### Chore

- chore: bump version to 2.8.2 (b3f6c72)

## [v2.8.1] - 2025-10-11

### Fixed

- fix: resolve index out of bounds panic and bump to v2.8.1 (772606f)

## [v2.8.0] - 2025-10-10

### Fixed

- fix: gate benchmarking binaries behind feature (0871d7a)

### Documentation

- docs: consolidate contributor and release guidance (4425df4)

### Chore

- chore: bump to v2.8.0 and cancel stale workflows (b2ba2aa)
- chore: cleanup gitignored detritus (eeacfea)
- chore: updated dependencies and added ai-rulez (d4f35d9)

### Other

- build(deps)(deps): bump the production-dependencies group with 5 updates (e5c3159)
- build(deps)(deps): bump serde in the production-dependencies group (05fb237)
- build(deps)(deps): bump tree-sitter-python in the tree-sitter group (ed1105f)

## [v2.7.0] - 2025-09-11

### Added

- feat: enhance linting tool comment preservation for all languages (5e9ed38)

### Fixed

- fix: ignore flaky network-dependent integration test in CI (f43c133)

### Changed

- refactor: cleanup and reorganize repository structure (f992978)

### Documentation

- docs: add GitHub Sponsors button to README (aa85949)

### Chore

- chore: bump version to v2.7.0 (fa1d016)

### Other

- ci(deps)(deps): bump actions/setup-python from 5 to 6 (9925521)
- ci(deps)(deps): bump actions/setup-node from 4 to 5 (c15f1f8)
- build(deps)(deps): bump clap in the production-dependencies group (041dac9)
- build(deps)(deps): bump the tree-sitter group with 4 updates (786574d)
- build(deps)(deps): bump regex in the production-dependencies group (e63598b)

## [v2.6.0] - 2025-08-23

### Added

- feat: implement CLI flag fixes and code cleanup for v2.6.0 (5e80837)

### Build

- ci: re-enable automatic package publishing on release (7f3f416)

### Other

- ci(deps)(deps): bump amannn/action-semantic-pull-request from 5 to 6 (b059c01)
- ci(deps)(deps): bump actions/checkout from 4 to 5 (b0a7dff)

## [v2.5.0] - 2025-08-13

### Fixed

- fix: update print statements for grammar cloning and compilation (6d81a6d)
- fix: correct output formatting for detected languages in CLI (1422456)

### Changed

- refactor: improve formatting and readability in integration test (274d74f)
- refactor: improve comment handling in processor (b326360)

### Testing

- test: add integration test for uncommenting code in multiple repositories (0fb3c6d)

### Chore

- chore: update repos.yaml by removing outdated repository URLs (f4fa8d5)
- chore: add formatting check to CI workflow (c9e0640)
- chore: update repos.yaml with additional repository URLs for integration testing (77a8fee)
- chore: add integration test repositories configuration (bd2f7da)
- chore: add serde_yaml dependency to Cargo.toml (5db6ec1)
- chore: update Cargo.lock to include serde_yaml and unsafe-libyaml dependencies (5897f55)
- chore: update .gitignore to include integration test repos cache directory (ebdcb46)

### Other

- Update all package versions to 2.5.0 and document Go documentation comment detection (8bbf5e4)
- Bump version to 2.5.0 (a5d2001)
- Implement Go documentation comment detection with extensible language handler architecture (69e8d8d)
- ci(deps)(deps): bump actions/download-artifact from 4 to 5 (90cdcf9)
- build(deps)(deps): bump the production-dependencies group with 3 updates (dc30793)
- build(deps)(deps): bump the production-dependencies group with 2 updates (244b25b)
- build(deps)(deps): bump the tree-sitter group with 2 updates (02ce191)
- build(deps)(deps): bump the production-dependencies group with 2 updates (1ce3678)
- build(deps)(deps): bump dirs from 5.0.1 to 6.0.0 (4343eed)

## [v2.4.2] - 2025-07-01

### Documentation

- docs: update README and CLAUDE.md with Homebrew installation (ae43f31)

## [v2.4.1] - 2025-07-01

### Added

- feat: bump version to v2.4.1 and finalize homebrew release pipeline (d77a20e)
- feat: update homebrew-tap submodule to v2.4.1-rc.3 (ca1a9ba)
- feat: implement Homebrew release pipeline (f3138d4)

### Fixed

- fix: handle existing releases in homebrew workflow (5d80c93)
- fix: resolve clippy::uninlined_format_args warnings for Rust 1.88 (2371d2f)

### Changed

- refactor: simplify Homebrew workflow to use source-based builds (cc5a4d9)

### Documentation

- docs: update README and CLAUDE.md with Homebrew installation (85dcfd9)

## [v2.4.0] - 2025-07-01

### Added

- feat: implement intelligent init command with tree-sitter grammar integration (7eb8130)
- feat: implement dynamic tree-sitter grammar loading system (7b1ba6c)
- feat: implement comprehensive TOML configuration system (269be1b)
- feat: add manual trigger support to publish workflow (19eaea3)

### Fixed

- fix: clippy uninlined format args warnings in config.rs (90acca0)
- fix: language-specific configuration for python docstrings (d1cffd2)
- fix: ensure `--version` matches what is in `Cargo.toml` (b16a09e)

### Documentation

- docs: update README and CLAUDE.md for v2.4.0 features (62ce35a)

### Chore

- chore: bump version to 2.4.0 (09ffa5c)
- chore: removed claude code review (9d9f9a9)

## [v2.3.1] - 2025-06-29

### Fixed

- fix: remove Zig dependency to enable crates.io publishing (v2.3.1) (b856b18)

## [v2.3.0] - 2025-06-29

### Added

- feat: add Cargo.lock to repository for Nix compatibility (6d71328)
- feat: add rc files support for shell/bash (e3568dd)
- feat: add haskell language support (3460452)
- feat: add shell/bash language support (847e0fb)
- feat: add zig language support (9fa1416)

### Fixed

- fix: respect parent .gitignore files when running from subdirectories (18fe916)
- fix: rebase and fix conflicts (079c0db)
- fix: rebase and fix confilcts (66d372f)
- fix: apply clippy fixes (d709cda)
- fix: apply clippy suggestions (94d0039)

### Chore

- chore: organise cargo imports (f892ac3)
- chore: organise cargo imports (752ade3)

### Other

- Claude Code Review workflow (30c7217)
- Claude PR Assistant workflow (1b55fca)
- doc: update `README.md` (36c8821)
- doc: update `README.md` (f20719e)
- doc: update `README.md` (8e207e4)

## [v2.2.3] - 2025-06-25

### Chore

- chore: bump version to 2.2.3 (409208f)

## [v2.2.2] - 2025-06-25

### Fixed

- fix: remove version suffix from release asset names to match download scripts (1593d37)

### Chore

- chore: bump version to 2.2.2 (030f519)

## [v2.2.1] - 2025-06-25

### Added

- feat: re-introduce pre-commit hooks with pip installation and git hooks documentation (a9183e1)

### Fixed

- fix: improve gitignore handling and fix npm/pypi download URLs (be0ff73)

### Documentation

- docs: update CLAUDE.md with multi-platform distribution learnings (0035813)

### Chore

- chore: applied pre-commit (5c37b4c)

## [v2.2.0] - 2025-06-25

### Added

- feat: change npm package name to uncomment-cli and update docs (1a61419)

### Fixed

- fix: handle case where npm version is already set in publish workflow (1bce920)

### Documentation

- docs: improve package descriptions and READMEs for npm and PyPI (225cc08)

### Chore

- chore: bump version to 2.2.0 for release (a668a18)
- chore: bump version to 2.1.1 and add .npmignore (85d100a)

## [v2.1.1-rc.7] - 2025-06-25

### Added

- feat: implement comprehensive package distribution system (5f79d22)

### Fixed

- fix: add missing fi in PyPI publish script (f528e4b)
- fix: change npm package name to uncomment-ast (be9bd21)
- fix: update npm install command in publish workflow (2bdb2a7)
- fix: change npm package name to @goldziher/uncomment to avoid naming conflict (7da9581)
- fix: add --allow-dirty flag to cargo publish command (b7e884c)

## [v2.1.1-rc.1] - 2025-06-24

### Added

- feat: add npm and pip package distribution with RC versioning (bd2668a)

### Fixed

- fix: align Python package version format with git tags (46feeca)

### Documentation

- docs: update README with v2.1.0 features and performance benchmarks (82739f1)

## [v2.1.0] - 2025-06-24

### Added

- feat: add benchmarking and profiling tools (f74d55a)
- feat: add support for YAML, HCL/Terraform, and Makefile languages (2ef8ec0)
- feat: expand file extension support for Python and TypeScript variants (c6cbcbe)
- feat: update all dependencies to latest versions and add parallel processing (d9db791)
- feat: enhance linting directive preservation and git repository handling (e69bf97)

### Documentation

- docs: update CLAUDE.md to reflect tree-sitter rewrite (79c045d)

### Chore

- chore: bump version to 2.1.0 (4057420)

## [v2.0.0] - 2025-06-23

### Added

- feat: complete tree-sitter based rewrite with enhanced language support (34d9d23)
- feat: Add recursive .gitignore parsing to uncomment tool (73c8b98)

### Changed

- refactor: remove dead code and simplify preservation rules (b6801ac)

### Documentation

- docs: add CLAUDE.md for AI assistant context (4f10076)

### Chore

- chore: applied formatting (2f47cfb)

### Other

- Fix: Resolve TypeScript regex mangling issue (#8) (d8bc465)

## [v1.0.5] - 2025-04-16

### Added

- feat: v1.0.5 (7f030f6)

### Chore

- chore: fixed doc string mangling (821ce0e)
- chore: fixed issues with typescript files (79aaf07)
- chore: removed comments (2839569)
- chore: fixed test mangling of rust code (8dd6641)
- chore: refactored codebase (8dbc343)
- chore: updated deps (70fb7c1)
- chore: add failing test (daaaa95)
- chore: Update README.md (8a9fe9f)

## [v1.0.4] - 2025-03-08

### Chore

- chore: fixed exit codes (8257feb)

## [v1.0.3] - 2025-03-08

### Added

- feat: v1.0.3 (5ecd708)

### Chore

- chore: fixed doc string removal (d617646)
- chore: switched to using regex (86f82b5)

## [v1.0.2] - 2025-03-08

### Chore

- chore: fix mangling issues (810dcf9)

## [v1.0.1] - 2025-03-08

### Chore

- chore: updated pre-commit hook (c222f35)
- chore: fix unique errors (417d503)
- chore: updated shell script (a4b7b91)
- chore: added installation script and pre-commit hooks (93d528c)

### Other

- Update readme.md (1aadf3f)

## [v1.0.0] - 2025-03-08

### Chore

- chore: downgraded edition (60fdda3)
- chore: added github workflows (9d243e4)
- chore: updated rust edition (2019bfb)

### Other

- initial (f4c7969)
