"""Verify release and source version drift without changing schema versions."""

import importlib.util
import tempfile
import unittest
from pathlib import Path


class PluginVersionTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location(
            "version_sync", Path(__file__).parents[1] / "sync_plugin_version.py"
        )
        self.module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.module)
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "plugin/.ai-rulez").mkdir(parents=True)
        (self.root / "Cargo.toml").write_text('[package]\nversion = "3.10.3"\n')
        self.config = self.root / "plugin/.ai-rulez/config.plugins.toml"
        self.config.write_text('version = "5.0"\n[plugin]\nversion = "3.1.0"\n')

    def test_should_reject_drift_without_changing_source_in_check_mode(self):
        original = self.config.read_bytes()
        self.assertEqual(self.module.synchronize(self.root, check=True, expect=None), 1)
        self.assertEqual(self.config.read_bytes(), original)
        self.assertEqual(self.module.synchronize(self.root, check=False, expect=None), 0)
        self.assertEqual(self.config.read_text(), 'version = "5.0"\n[plugin]\nversion = "3.10.3"\n')
        self.assertEqual(self.module.synchronize(self.root, check=True, expect="3.10.3"), 0)

    def test_should_reject_release_tag_drift_without_writing(self):
        self.module.synchronize(self.root, check=False, expect=None)
        original = self.config.read_bytes()
        self.assertEqual(self.module.synchronize(self.root, check=False, expect="3.11.0"), 1)
        self.assertEqual(self.config.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
