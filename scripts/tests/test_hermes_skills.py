"""Verify skill discovery through the actual Hermes Agent plugin manager."""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROBE = """
from hermes_cli.plugins import PluginManager
manager = PluginManager()
manager.discover_and_load()
names = manager.list_plugin_skills("uncomment")
assert names == ["uncomment"], f"registered skill count/name mismatch: {names}"
skill = manager.find_plugin_skill("uncomment:uncomment")
assert skill is not None and skill.is_file(), "registered skill is unreadable"
references = sorted((skill.parent / "references").glob("*.md"))
assert len(references) == 2, f"expected two skill references, received {len(references)}"
assert all(path.stat().st_size > 100 for path in references), "empty skill reference"
print("Hermes native discovery: one skill, two readable references")
"""


class HermesSkillTests(unittest.TestCase):
    """Exercise the installed plugin bytes with isolated Hermes state."""

    def probe(self, remove_skill: bool) -> subprocess.CompletedProcess[str]:
        """Use a fresh plugin installation and SDK subprocess for each test."""
        with tempfile.TemporaryDirectory(prefix="uncomment-hermes-contract-") as temporary:
            home = Path(temporary)
            plugin = home / "plugins/uncomment"
            shutil.copytree(ROOT / "plugin/.hermes/plugins/uncomment", plugin)
            (home / "config.yaml").write_text("plugins:\n  enabled: [uncomment]\n")
            bundled = home / "empty-bundled"
            bundled.mkdir()
            if remove_skill:
                (plugin / "skills/uncomment/SKILL.md").unlink()
            environment = os.environ | {
                "HERMES_HOME": str(home),
                "HERMES_BUNDLED_PLUGINS": str(bundled),
                "HERMES_ENABLE_PROJECT_PLUGINS": "true",
            }
            return subprocess.run(
                [sys.executable, "-c", PROBE], cwd=home, env=environment, text=True, capture_output=True, check=False
            )

    def test_should_discover_installed_uncomment_skill_and_references(self) -> None:
        """A native registration succeeds with all bundled resources readable."""
        result = self.probe(remove_skill=False)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("one skill, two readable references", result.stdout)

    def test_should_reject_an_installation_missing_its_registered_skill(self) -> None:
        """The contract fails for missing installed bytes rather than passing empty."""
        result = self.probe(remove_skill=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("registered skill count/name mismatch", result.stderr)


if __name__ == "__main__":
    unittest.main()
