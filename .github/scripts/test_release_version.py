"""Protect dependency locks and reject malformed release tags before file changes."""
import contextlib
import importlib.util
import io
import os
import shutil
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("release_version", Path(__file__).with_name("set-release-version.py"))
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ReleaseVersionTests(unittest.TestCase):
    def setUp(self):
        self.previous_directory = Path.cwd()
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.addCleanup(os.chdir, self.previous_directory)
        os.chdir(self.temporary.name)
        for file in ["Cargo.toml", "Cargo.lock", "crates/core/Cargo.toml", "crates/desktop/Cargo.toml"]:
            Path(file).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / file, file)

    def files(self):
        return [Path(name).read_bytes() for name in ["Cargo.toml", "Cargo.lock"]]

    def test_versions_change_without_dependency_updates(self):
        for tag in ["v1.2.3", "v2.0.0-rc.1", "v2.0.0-1+build.42"]:
            with self.subTest(tag=tag), contextlib.redirect_stdout(io.StringIO()):
                before = tomllib.loads(Path("Cargo.lock").read_text())
                MODULE.set_version(tag)
                manifest = tomllib.loads(Path("Cargo.toml").read_text())
                self.assertEqual(manifest["workspace"]["package"]["version"], tag[1:])
                after = tomllib.loads(Path("Cargo.lock").read_text())
                for package in before["package"]:
                    if package["name"] in {"wh3-core", "wh3-mod-manager"}:
                        package["version"] = tag[1:]
                self.assertEqual(before, after)

    def test_invalid_tags_do_not_modify_files(self):
        before = self.files()
        for tag in ["main", "v1", "v01.2.3", "v1.2.3-01", "v1.2.3;echo bad", "v1.2.3/../../main", "v1.2.3\n"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                MODULE.set_version(tag)
            self.assertEqual(before, self.files())

    def test_missing_lock_entry_does_not_partially_update_manifest(self):
        lock = Path("Cargo.lock")
        lock.write_text(lock.read_text().replace('name = "wh3-core"', 'name = "unexpected"'))
        before = self.files()
        with self.assertRaises(ValueError):
            MODULE.set_version("v1.2.3")
        self.assertEqual(before, self.files())


if __name__ == "__main__":
    unittest.main()
