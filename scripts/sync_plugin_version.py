"""Keep plugin release versions synchronized with the CLI package."""

import argparse
import re
import sys
from pathlib import Path

import tomllib

VERSION = re.compile(r'(\[plugin\][^\[]*?\nversion = ")([^"\n]*)(")', re.DOTALL)


def synchronize(root: Path, *, check: bool, expect: str | None) -> int:
    """Reject drift read-only, or update only the plugin version source."""
    core = tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"]
    config = root / "plugin/.ai-rulez/config.plugins.toml"
    original = config.read_text()
    plugin = tomllib.loads(original)["plugin"]["version"]
    wanted = expect or core
    if core == wanted and plugin == wanted:
        return 0
    if check or expect is not None:
        sys.stderr.write(f"Version drift: CLI={core}, plugin={plugin}, expected={wanted}\n")
        return 1
    updated, count = VERSION.subn(lambda match: match[1] + core + match[3], original, count=1)
    if count != 1:
        raise ValueError("Expected exactly one [plugin].version source")
    config.write_text(updated)
    return 0


def main() -> int:
    """Check release tags or synchronize the native plugin configuration."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--expect")
    arguments = parser.parse_args()
    return synchronize(Path(__file__).resolve().parents[1], check=arguments.check, expect=arguments.expect)


if __name__ == "__main__":
    raise SystemExit(main())
