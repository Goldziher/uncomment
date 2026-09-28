//! Which occurrences of a tag word `uncomment lint` treats as tags, driven through the compiled
//! binary over fixture repositories with a real `.git/HEAD`.
//!
//! Every fixture line here is shaped after a false positive or a true positive seen in a real
//! monorepo run, so the assertions are on the exact set of `(file, line, tag)` sites reported —
//! a missing true positive fails as loudly as a surviving false one.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const ENABLED: &str = "[lint]\nenabled = true\n";

/// The key `--fix --todo-key` inserts; any key the default `key_pattern` accepts would do.
const FIX_KEY: &str = "AMVP-9";

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_uncomment"))
}

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(&path, content).expect("write fixture file");
}

fn read(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative)).expect("read fixture file")
}

fn fixture(config: &str) -> TempDir {
    let temp = TempDir::new().expect("temp dir");
    write(temp.path(), ".git/HEAD", "ref: refs/heads/work\n");
    write(temp.path(), ".uncomment.toml", config);
    temp
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run uncomment")
}

/// Every reported `(path, line, tag)`, deduplicated across rules: a site is a tag or it is not.
fn reported_sites(dir: &Path) -> BTreeSet<(String, u64, String)> {
    let output = run(dir, &["lint", "--format", "json", "."]);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "lint --format json is JSON ({error}): stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    report["violations"]
        .as_array()
        .expect("violations array")
        .iter()
        .map(|violation| {
            (
                violation["path"].as_str().expect("path").to_string(),
                violation["line"].as_u64().expect("line"),
                violation["tag"].as_str().expect("tag").to_string(),
            )
        })
        .collect()
}

fn site(path: &str, line: u64, tag: &str) -> (String, u64, String) {
    (path.to_string(), line, tag.to_string())
}

/// Prose, placeholders and ruff rule descriptions from a real monorepo: each line mentions a tag
/// word and none of them is a tag.
const PROSE_PY: &str = "\
# usage: /api/devices/daily/aggregations/column/_search?tenant_id=XXX&utc_offset=2
# Line contains TODO
# Line contains XXX
# Missing author in TODO
# Invalid TODO tag: `FIXME`
# needs — canonical TODO, case-insensitive matching, a key pattern that accepts
# returns {\"uuid\": XXX} for the next page
value = 1
";

const PROSE_TOML: &str = "\
[lint]
ignore = [
  \"FIX001\", # Line contains FIXME, consider resolving the issue
  \"TD001\",  # Invalid TODO tag: `FIXME`
]
";

const PROSE_JSONC: &str = "\
{
  // `// TODO AMVP-...` markers — same model as ruff.toml per-file-ignores.
  \"rules\": {}
}
";

#[test]
fn a_tag_word_in_prose_quotes_or_a_url_is_never_reported() {
    let temp = fixture(ENABLED);
    let root = temp.path();
    write(root, "prose.py", PROSE_PY);
    write(root, "ruff.toml", PROSE_TOML);
    write(root, ".oxlintrc.jsonc", PROSE_JSONC);

    let sites = reported_sites(root);
    assert!(sites.is_empty(), "prose must not be reported: {sites:?}");
}

/// The true positives, one per written form, each on a known line.
const TAGS_PY: &str = "\
# TODO: x
# TODO(AMVP-1): keyed, so clean
# TODO AMVP-1 x
# FIXME - x
# XXX: x
# HACK x
# todo: x
value = 1  # TODO x
print(value)  # noqa: T201  # TODO: fix T201
# Socks handling - TODO: create a failure type
";

const TAGS_TSX: &str = "\
// TODO x
/* TODO x */
export const View = () => <div>{/* TODO x */}</div>;
";

#[test]
fn a_tag_in_tag_position_is_reported_in_every_written_form() {
    let temp = fixture(ENABLED);
    let root = temp.path();
    write(root, "tags.py", TAGS_PY);
    write(root, "view.tsx", TAGS_TSX);

    let expected: BTreeSet<_> = [
        site("tags.py", 1, "TODO"),
        site("tags.py", 3, "TODO"),
        site("tags.py", 4, "FIXME"),
        site("tags.py", 5, "XXX"),
        site("tags.py", 6, "HACK"),
        site("tags.py", 7, "todo"),
        site("tags.py", 8, "TODO"),
        site("tags.py", 9, "TODO"),
        site("tags.py", 10, "TODO"),
        site("view.tsx", 1, "TODO"),
        site("view.tsx", 2, "TODO"),
        site("view.tsx", 3, "TODO"),
    ]
    .into_iter()
    .collect();
    assert_eq!(reported_sites(root), expected);
}

#[test]
fn fix_rewrites_tags_and_leaves_prose_byte_identical() {
    let temp = fixture(ENABLED);
    let root = temp.path();
    write(root, "prose.py", PROSE_PY);
    write(root, "ruff.toml", PROSE_TOML);
    write(
        root,
        "mixed.py",
        "# FIXME: x\n# Line contains FIXME\nvalue = 1  # noqa: E501  # HACK y\n",
    );

    let output = run(root, &["lint", "--fix", "--todo-key", FIX_KEY, "."]);
    assert!(
        output.status.success(),
        "every violation is fixable: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(read(root, "prose.py"), PROSE_PY);
    assert_eq!(read(root, "ruff.toml"), PROSE_TOML);
    assert_eq!(
        read(root, "mixed.py"),
        "# TODO(AMVP-9): x\n# Line contains FIXME\nvalue = 1  # noqa: E501  # TODO(AMVP-9): y\n"
    );
}

/// A tag word in each documentation syntax, next to plain comments that must stay in scope — the
/// Go one included, which its handler files as documentation only because it precedes a `func`.
const DOCS_PY: &str = "\
def fetch():
    \"\"\"Returns the TODO list; FIXME: stale after a sync.\"\"\"
    # TODO: plain comment inside the body
    return []
";

const DOCS_RS: &str = "\
/// TODO: document the error cases
//! FIXME: crate docs
// TODO: plain comment
fn main() {}
";

const DOCS_TS: &str = "\
/** TODO: describe the return value */
export const run = () => 1; // TODO: plain comment
";

const DOCS_GO: &str = "\
package main

// TODO: split this function
func main() {}
";

#[test]
fn doc_comments_and_docstrings_are_skipped_but_plain_comments_are_not() {
    let temp = fixture(ENABLED);
    let root = temp.path();
    write(root, "docs.py", DOCS_PY);
    write(root, "docs.rs", DOCS_RS);
    write(root, "docs.ts", DOCS_TS);
    write(root, "docs.go", DOCS_GO);

    let expected: BTreeSet<_> = [
        site("docs.py", 3, "TODO"),
        site("docs.rs", 3, "TODO"),
        site("docs.ts", 2, "TODO"),
        site("docs.go", 3, "TODO"),
    ]
    .into_iter()
    .collect();
    assert_eq!(reported_sites(root), expected);
}

#[test]
fn include_doc_comments_reports_tags_in_documentation_too() {
    let temp = fixture("[lint]\nenabled = true\ninclude_doc_comments = true\n");
    let root = temp.path();
    write(root, "docs.py", DOCS_PY);
    write(root, "docs.rs", DOCS_RS);
    write(root, "docs.ts", DOCS_TS);

    let expected: BTreeSet<_> = [
        site("docs.py", 2, "FIXME"),
        site("docs.py", 3, "TODO"),
        site("docs.rs", 1, "TODO"),
        site("docs.rs", 2, "FIXME"),
        site("docs.rs", 3, "TODO"),
        site("docs.ts", 1, "TODO"),
        site("docs.ts", 2, "TODO"),
    ]
    .into_iter()
    .collect();
    assert_eq!(reported_sites(root), expected);
}

#[test]
fn fix_leaves_doc_comments_byte_identical_by_default() {
    let temp = fixture(ENABLED);
    let root = temp.path();
    write(root, "docs.py", DOCS_PY);
    write(root, "docs.rs", DOCS_RS);

    let output = run(root, &["lint", "--fix", "--todo-key", FIX_KEY, "."]);
    assert!(
        output.status.success(),
        "every violation is fixable: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        read(root, "docs.py"),
        DOCS_PY.replace("# TODO: plain", "# TODO(AMVP-9): plain")
    );
    assert_eq!(
        read(root, "docs.rs"),
        DOCS_RS.replace("// TODO: plain", "// TODO(AMVP-9): plain")
    );
}
