"""Set first-party package versions from a release tag without updating dependencies."""
import os
import re
import sys
import tomllib
from pathlib import Path


def set_version(tag: str) -> None:
    number = r"(?:0|[1-9][0-9]*)"
    identifier = rf"(?:{number}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
    semver = rf"{number}\.{number}\.{number}(?:-{identifier}(?:\.{identifier})*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    if not re.fullmatch(rf"v{semver}", tag, flags=re.ASCII):
        raise ValueError("Expected a SemVer tag such as v1.2.3 or v1.2.3-rc.1")
    version = tag[1:]
    manifest_path = Path("Cargo.toml")
    manifest = manifest_path.read_text(encoding="utf-8")
    workspace = tomllib.loads(manifest)["workspace"]
    names = set()
    for member in workspace["members"]:
        package = tomllib.loads(Path(member, "Cargo.toml").read_text(encoding="utf-8"))["package"]
        if package.get("version") != {"workspace": True}:
            raise ValueError(f"Package {package['name']} must inherit the workspace version")
        names.add(package["name"])
    updated_manifest, count = re.subn(
        r'(?ms)(^\[workspace\.package\]\s*\n(?:(?!^\[).)*?^version\s*=\s*")[^"]+("\s*$)',
        lambda match: match[1] + version + match[2], manifest,
    )
    if count != 1:
        raise ValueError("Expected exactly one workspace.package.version")
    lock_path = Path("Cargo.lock")
    lock = lock_path.read_text(encoding="utf-8")
    for name in names:
        lock, count = re.subn(
            rf'(?m)(^name = "{re.escape(name)}"\nversion = ")[^"]+("$)',
            lambda match: match[1] + version + match[2], lock,
        )
        if count != 1:
            raise ValueError(f"Expected exactly one lockfile entry for {name}")
    manifest_path.write_text(updated_manifest, encoding="utf-8")
    lock_path.write_text(lock, encoding="utf-8")
    print(f"Release version: {version}")


if __name__ == "__main__":
    try:
        set_version(os.environ["RELEASE_TAG"])
    except (KeyError, ValueError) as error:
        sys.exit(str(error))
