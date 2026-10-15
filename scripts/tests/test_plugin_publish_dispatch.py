import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/publish.yaml"


class PluginDispatchTests(unittest.TestCase):
    def test_plugin_only_mode_is_guarded_and_uses_dispatch_commit(self) -> None:
        text = WORKFLOW.read_text()
        self.assertIn("plugins-only:", text)
        self.assertIn("if: steps.npm-version.outputs.exists != 'true'", text)
        self.assertIn("if error.code != 404:", text)
        blocks = dict(
            re.findall(r"^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)", text.split("jobs:\n", 1)[1], re.MULTILINE | re.DOTALL)
        )
        self.assertGreater(len(blocks), 8)
        for name, block in blocks.items():
            if name == "plugin-meta" or "report" in name:
                continue
            if "plugin" in name and ("opencode" in name or "npm" in name or "hermes" in name):
                self.assertIn("plugin-meta", block)
                self.assertIn("needs.plugin-meta.result", block)
                self.assertIn("inputs.plugins-only && github.sha", block)
                self.assertIn("actions/publish-", block)
            else:
                self.assertIn("!inputs.plugins-only", block, name)

    def test_npm_preflight_skips_only_matching_versions_and_fails_closed(self) -> None:
        text = WORKFLOW.read_text()
        script = (
            text.split("        id: npm-version\n", 1)[1]
            .split("          python3 - <<'PY'\n", 1)[1]
            .split("          PY", 1)[0]
        )
        script = "\n".join(line[10:] for line in script.splitlines())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "plugin").mkdir()
            (root / "plugin/package.json").write_text('{"name":"@example/plugin","version":"1.2.3"}')
            cases = [
                ('{"name":"@example/plugin","version":"1.2.3"}', 200, 0, "exists=true\n"),
                ("{}", 404, 0, "exists=false\n"),
                ("{}", 500, 1, None),
                ('{"name":"wrong","version":"1.2.3"}', 200, 1, None),
            ]
            for payload, status, expected, output in cases:
                with self.subTest(payload=payload, status=status):
                    prelude = (
                        "import io, urllib.error, urllib.request\n"
                        "def request(*args, **kwargs):\n"
                        f'    if {status} != 200: raise urllib.error.HTTPError("url", {status}, "test", None, None)\n'
                        f"    return io.StringIO({payload!r})\n"
                        "urllib.request.urlopen = request\n"
                    )
                    target = root / f"output-{status}-{expected}"
                    env = dict(os.environ, GITHUB_OUTPUT=str(target))
                    result = subprocess.run(
                        ["python3", "-c", prelude + script], cwd=root, env=env, capture_output=True, text=True
                    )
                    self.assertEqual(result.returncode, expected, result.stderr)
                    if output is not None:
                        self.assertEqual(target.read_text(), output)

    def test_metadata_rejects_version_mismatch_and_unsafe_ref(self) -> None:
        text = WORKFLOW.read_text()
        block = text.split("  plugin-meta:\n", 1)[1].split("  meta:\n", 1)[0]
        script = block.split("          python3 - <<'PY'\n", 1)[1].split("          PY", 1)[0]
        script = "\n".join(line[10:] for line in script.splitlines())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "plugin/.hermes/package").mkdir(parents=True)
            (root / "Cargo.toml").write_text('[package]\nversion = "1.2.3"\n')
            (root / "plugin/package.json").write_text('{"version":"1.2.3"}')
            manifest = root / "plugin/.hermes/package/pyproject.toml"
            manifest.write_text('[project]\nversion = "1.2.3"\n')
            env = dict(os.environ, GITHUB_OUTPUT=str(root / "output"), DISPATCH_REF="refs/heads/main")
            result = subprocess.run(["python3", "-c", script], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((root / "output").read_text(), "version=1.2.3\n")
            manifest.write_text('[project]\nversion = "1.2.4"\n')
            result = subprocess.run(["python3", "-c", script], cwd=root, env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Plugin versions disagree", result.stderr)
            env["DISPATCH_REF"] = "refs/heads/other"
            result = subprocess.run(["python3", "-c", script], cwd=root, env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("requires main", result.stderr)


if __name__ == "__main__":
    unittest.main()
