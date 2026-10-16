"""Print the desktop changelog entry for a release tag as GitHub Release notes.

The changelog file starts with a heading and a note for developers, and every release in the
fixed version group lists the core crate under "Patch changes". Neither belongs in the notes
users read, so only the entry's own sections are printed.
"""
import re
import sys
from pathlib import Path

CHANGELOG = Path("crates/desktop/CHANGELOG.md")
DEPENDENCY_LINE = re.compile(r"- Updated dependencies: \S+")


def notes(changelog: str, tag: str) -> str:
    version = tag.removeprefix("v")
    entries = re.split(r"(?m)^## ", changelog)[1:]
    entry = next((entry for entry in entries if entry.split(maxsplit=1)[0] == version), None)
    if entry is None:
        raise ValueError(f"{CHANGELOG} has no entry for {version}")
    sections = re.split(r"(?m)^(?=### )", entry.split("\n", 1)[1] if "\n" in entry else "")
    kept = []
    for section in sections:
        if not section.startswith("### "):
            continue
        items = [line for line in section.splitlines()[1:] if line.strip()]
        if items and all(DEPENDENCY_LINE.fullmatch(line) for line in items):
            continue
        kept.append(section.strip())
    if not kept:
        raise ValueError(f"The {version} entry has no release notes")
    return "\n\n".join(kept) + "\n"


def main() -> int:
    if len(sys.argv) != 2:
        print("Usage: release-notes.py v<version>", file=sys.stderr)
        return 2
    try:
        sys.stdout.write(notes(CHANGELOG.read_text(encoding="utf-8"), sys.argv[1]))
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
