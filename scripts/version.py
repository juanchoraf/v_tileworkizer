"""The only explicit version change entry point. Packaging reads Cargo.toml."""
from pathlib import Path
import re
import sys
import tomllib


def main():
    if len(sys.argv) != 2 or not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", sys.argv[1]):
        raise SystemExit("Expected a stable MAJOR.MINOR.PATCH version")
    version = sys.argv[1]
    parts = tuple(map(int, version.split(".")))
    if not (parts[0] <= 255 and parts[1] <= 255 and parts[2] <= 65535):
        raise SystemExit("Version exceeds Windows Installer limits: 255.255.65535")
    path = Path(__file__).resolve().parents[1] / "Cargo.toml"
    text = path.read_text()
    old = tomllib.loads(text)["package"]["version"]
    if parts <= tuple(map(int, old.split("."))):
        raise SystemExit("New version must be greater than the current version")
    before, after = text.split("[package]", 1)
    package, separator, rest = after.partition("\n[")
    package, count = re.subn(r'^version = "[^"]+"$', f'version = "{version}"', package, count=1, flags=re.M)
    if count != 1:
        raise SystemExit("Cannot locate the package version")
    lock = path.with_name("Cargo.lock")
    lock_text = None
    if lock.exists():
        lock_text, matches = re.subn(
            r'(\[\[package\]\]\nname = "v_tileworkizer"\nversion = ")[^"]+(")',
            lambda m: m.group(1) + version + m.group(2), lock.read_text(), count=1)
        if matches != 1:
            raise SystemExit("Cannot locate v_tileworkizer in Cargo.lock")
        tomllib.loads(lock_text)
    path.write_text(before + "[package]" + package + separator + rest)
    if lock_text is not None:
        lock.write_text(lock_text)


if __name__ == "__main__":
    main()
