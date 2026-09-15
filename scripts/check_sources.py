"""Validate source without compiling or invoking any build/installer tooling."""
import ast
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib
import xml.etree.ElementTree as ET
import zlib

ROOT = Path(__file__).resolve().parents[1]


def check_version():
    with tempfile.TemporaryDirectory(prefix="v_tileworkizer-version-test-") as directory:
        root = Path(directory)
        (root / "scripts").mkdir()
        shutil.copy2(ROOT / "scripts/version.py", root / "scripts/version.py")
        manifest = '[package]\nname = "v_tileworkizer"\nversion = "1.2.3"\n\n[dependencies]\nanyhow = "1"\n'
        lock = 'version = 4\n\n[[package]]\nname = "v_tileworkizer"\nversion = "1.2.3"\n\n[[package]]\nname = "other"\nversion = "1.2.3"\n'
        (root / "Cargo.toml").write_text(manifest)
        (root / "Cargo.lock").write_text(lock)
        for invalid in ["1.2.3", "1.0.0", "256.0.0", "2.0.65536", "2.0.0;whoami", "02.0.0"]:
            result = subprocess.run([sys.executable, root / "scripts/version.py", invalid], capture_output=True)
            assert result.returncode != 0, invalid
            assert (root / "Cargo.toml").read_text() == manifest
            assert (root / "Cargo.lock").read_text() == lock
        subprocess.run([sys.executable, root / "scripts/version.py", "2.0.0"], check=True)
        assert tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"] == "2.0.0"
        packages = tomllib.loads((root / "Cargo.lock").read_text())["package"]
        assert packages[0]["version"] == "2.0.0"
        assert packages[1]["version"] == "1.2.3"


def check_icons():
    ET.parse(ROOT / "assets/logo.svg")
    png = (ROOT / "assets/logo.png").read_bytes()
    assert png[:8] == b"\x89PNG\r\n\x1a\n"
    pos = 8
    while pos < len(png):
        length = struct.unpack(">I", png[pos:pos + 4])[0]
        chunk = png[pos + 4:pos + 8 + length]
        assert zlib.crc32(chunk) == struct.unpack(">I", png[pos + 8 + length:pos + 12 + length])[0]
        pos += length + 12
    assert pos == len(png)
    ico = (ROOT / "assets/logo.ico").read_bytes()
    assert struct.unpack("<HHH", ico[:6]) == (0, 1, 1)
    assert ico[22:] == png
    icns = (ROOT / "assets/logo.icns").read_bytes()
    assert icns[:4] == b"icns" and struct.unpack(">I", icns[4:8])[0] == len(icns)
    for name in ["banner.bmp", "dialog.bmp"]:
        bmp = (ROOT / "assets" / name).read_bytes()
        assert bmp[:2] == b"BM" and struct.unpack("<I", bmp[2:6])[0] == len(bmp)


def main():
    source = [ROOT / "build.rs"] + list((ROOT / "src").rglob("*.rs")) + list((ROOT / "scripts").glob("*.py")) + list((ROOT / "scripts").glob("*.sh"))
    for path in source:
        text = path.read_text()
        assert len(text.splitlines()) <= 600, f"File exceeds 600 lines: {path}"
        if path.suffix == ".py":
            ast.parse(text, filename=str(path))
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    tomllib.loads((ROOT / "rust-toolchain.toml").read_text())
    tomllib.loads((ROOT / ".cargo/config.toml").read_text())
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    assert next(p for p in lock["package"] if p["name"] == "v_tileworkizer")["version"] == manifest["package"]["version"]
    check_version()
    check_icons()
    from check_packaging import check
    check()
    subprocess.run(["rustfmt", "--check", "--edition", "2024", "src/main.rs", "build.rs"], cwd=ROOT, check=True)
    if shutil.which("bash"):
        subprocess.run(["bash", "-n", "scripts/build_binaries.sh", "scripts/bump_version.sh"], cwd=ROOT, check=True)
    print(f"Source checks passed ({len(source)} source files). No builds or native runtime tests were run.")


if __name__ == "__main__":
    main()
