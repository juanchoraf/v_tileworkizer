"""Build one native installer on the host OS. Never executed by installed users."""
import argparse
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def run(*args, **kwargs):
    subprocess.run([str(a) for a in args], check=True, **kwargs)


def copy(source, dest):
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, dest)


def write(dest, text):
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(text, encoding="utf-8")


def unix_payload(stage, binary, prefix):
    copy(binary, stage / prefix / "bin/v_tileworkizer")
    (stage / prefix / "bin/v_tileworkizer").chmod(0o755)
    copy(ROOT / "packaging/v_tileworkizer.desktop", stage / prefix / "share/applications/v_tileworkizer.desktop")
    copy(ROOT / "assets/logo.svg", stage / prefix / "share/icons/hicolor/scalable/apps/v_tileworkizer.svg")
    copy(ROOT / "assets/logo.png", stage / prefix / "share/icons/hicolor/256x256/apps/v_tileworkizer.png")
    for name in ["LICENSE-MIT", "LICENSE-APACHE", "README.md"]:
        copy(ROOT / name, stage / prefix / "share/doc/v_tileworkizer" / name)
    autostart = "usr/local/etc/xdg/autostart" if prefix == "usr/local" else "etc/xdg/autostart"
    copy(ROOT / "packaging/autostart.desktop", stage / autostart / "v_tileworkizer.desktop")


def linux(stage, binary, output, version, arch, kind):
    unix_payload(stage, binary, "usr")
    if kind == "deb":
        deb_arch = {"x86_64": "amd64", "aarch64": "arm64"}[arch]
        write(stage.parent / "debian/control", "Source: v-tileworkizer\nSection: utils\nPriority: optional\nMaintainer: juanchoraf <juanchoraf@users.noreply.github.com>\n\nPackage: v-tileworkizer\nArchitecture: any\nDescription: Workspace organizer\n")
        shlibs = subprocess.check_output(["dpkg-shlibdeps", "-O", "-e" + str(binary)], cwd=stage.parent, text=True)
        dependencies = next(line.split("=", 1)[1] for line in shlibs.splitlines() if line.startswith("shlibs:Depends="))
        write(stage / "DEBIAN/control", f"""Package: v-tileworkizer
Version: {version}
Architecture: {deb_arch}
Maintainer: juanchoraf <juanchoraf@users.noreply.github.com>
Section: utils
Priority: optional
Depends: {dependencies}, libx11-6, libxrandr2, libxcursor1, libxi6, libgl1, libxkbcommon0, libwayland-client0
Homepage: https://github.com/juanchoraf/v_tileworkizer
Description: Native tiling workspace organizer
 A desktop GUI and persistent session service for EWMH X11 desktops.
""")
        run("dpkg-deb", "--root-owner-group", "--build", stage, output)
    else:
        top = stage.parent / "rpmbuild"
        top.mkdir()
        spec = top / "app.spec"
        stage_quoted = "'" + str(stage).replace("'", "'\\''") + "'"
        write(spec, f"""Name: v_tileworkizer
Version: {version}
Release: 1
Summary: Native tiling workspace organizer
License: MIT OR Apache-2.0
URL: https://github.com/juanchoraf/v_tileworkizer
Requires: libX11.so.6()(64bit), libXrandr.so.2()(64bit), libXcursor.so.1()(64bit), libXi.so.6()(64bit), libxkbcommon.so.0()(64bit), libGL.so.1()(64bit)
AutoReqProv: yes
%description
A native desktop GUI and persistent session service for EWMH X11 desktops.
%install
mkdir -p %{{buildroot}}
cp -a {stage_quoted}/. %{{buildroot}}/
%files
/usr/bin/v_tileworkizer
/usr/share/applications/v_tileworkizer.desktop
/usr/share/icons/hicolor/scalable/apps/v_tileworkizer.svg
/usr/share/icons/hicolor/256x256/apps/v_tileworkizer.png
/usr/share/doc/v_tileworkizer/
/etc/xdg/autostart/v_tileworkizer.desktop
""")
        run("rpmbuild", "-bb", "--define", f"_topdir {top}", "--define", "_build_id_links none", "--define", "debug_package %{nil}", spec)
        packages = list((top / "RPMS").rglob("*.rpm"))
        if len(packages) != 1:
            raise RuntimeError("Expected exactly one RPM")
        copy(packages[0], output)


def freebsd(stage, binary, output, version, arch):
    import json
    unix_payload(stage, binary, "usr/local")
    metadata = stage.parent / "metadata"
    metadata.mkdir()
    deps = {}
    for name in ["libX11", "libXrandr", "libXcursor", "libXi", "libxkbcommon", "mesa-libs"]:
        info = subprocess.check_output(["pkg", "query", "%n|%v|%o", name], text=True).strip()
        package_name, package_version, origin = info.split("|")
        deps[package_name] = {"origin": origin, "version": package_version}
    # pkg computes checksums from the payload; all files are owned by root:wheel.
    manifest = {"name": "v_tileworkizer", "version": version, "origin": "x11-wm/v_tileworkizer",
        "comment": "Native tiling workspace organizer", "desc": "Desktop GUI and X11 session worker",
        "maintainer": "juanchoraf@users.noreply.github.com", "www": "https://github.com/juanchoraf/v_tileworkizer",
        "prefix": "/usr/local", "licenselogic": "dual", "licenses": ["MIT", "APACHE20"],
        "deps": deps,
        "files": {"/" + str(p.relative_to(stage)): "-" for p in stage.rglob("*") if p.is_file()}}
    write(metadata / "+MANIFEST", json.dumps(manifest))
    run("pkg", "create", "-r", stage, "-m", metadata, "-o", output.parent)
    generated = output.parent / f"v_tileworkizer-{version}.pkg"
    generated.replace(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--format", choices=["deb", "rpm", "pkg", "msi"])
    parser.add_argument("--refresh-dependencies", action="store_true", help="Refresh the Cargo lockfile to latest compatible releases")
    args = parser.parse_args()
    os.chdir(ROOT)
    system = {"Darwin": "macos", "Windows": "windows", "Linux": "linux", "FreeBSD": "freebsd"}.get(platform.system())
    rust_info = subprocess.check_output(["rustc", "-vV"], text=True)
    host = next(line.removeprefix("host: ") for line in rust_info.splitlines() if line.startswith("host: "))
    arch = host.split("-", 1)[0]
    if arch not in ["x86_64", "aarch64"]:
        arch = None
    if not system or not arch:
        raise SystemExit("Native packaging currently supports Linux/macOS/Windows/FreeBSD on x86_64/aarch64")
    kind = args.format or {"windows": "msi", "macos": "pkg", "freebsd": "pkg", "linux": "deb"}[system]
    if kind not in {"linux": ["deb", "rpm"], "windows": ["msi"], "macos": ["pkg"], "freebsd": ["pkg"]}[system]:
        raise SystemExit("Installer format does not match this host OS")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if args.refresh_dependencies:
        run("cargo", "update")
    if not (ROOT / "Cargo.lock").exists():
        run("cargo", "generate-lockfile")
    run("cargo", "build", "--release", "--locked", "--target", host, "--target-dir", ROOT / "target")
    binary = ROOT / "target" / host / "release" / ("v_tileworkizer.exe" if system == "windows" else "v_tileworkizer")
    output = ROOT / "dist" / f"v_tileworkizer-{version}-{system}-{arch}.{kind}"
    output.parent.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="v_tileworkizer-") as temporary:
        stage = Path(temporary) / "payload"
        stage.mkdir()
        if system == "linux":
            linux(stage, binary, output, version, arch, kind)
        elif system == "freebsd":
            freebsd(stage, binary, output, version, arch)
        else:
            from package_desktop import macos, windows
            (macos if system == "macos" else windows)(stage, binary, output, version, arch)
    print(output)


if __name__ == "__main__":
    main()
