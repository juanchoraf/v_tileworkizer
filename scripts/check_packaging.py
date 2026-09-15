"""Exercise declarative installer authoring with all build commands mocked out."""
import os
from pathlib import Path
import plistlib
import tempfile
from unittest.mock import patch
import xml.etree.ElementTree as ET
import package
import package_desktop


def check():
    with tempfile.TemporaryDirectory(prefix="v_tileworkizer-authoring-test-") as directory:
        root = Path(directory)
        binary = root / "fixture-binary"
        binary.write_bytes(b"source-check fixture; not an executable")
        deb = root / "debian-payload"
        deb.mkdir()
        with patch.object(package, "run") as run, patch.object(package.subprocess, "check_output", return_value="shlibs:Depends=libc6 (>= 2.39), libgcc-s1\n"):
            package.linux(deb, binary, root / "unused.deb", "1.2.3", "x86_64", "deb")
            assert run.call_args.args[0] == "dpkg-deb"
        assert "libc6 (>= 2.39)" in (deb / "DEBIAN/control").read_text()
        assert (deb / "etc/xdg/autostart/v_tileworkizer.desktop").exists()
        assert not any((deb / "DEBIAN" / name).exists() for name in ["preinst", "postinst", "prerm", "postrm"])
        mac = root / "mac-payload"
        mac.mkdir()
        with patch.dict(os.environ, {}, clear=True), patch.object(package_desktop, "run") as run:
            package_desktop.macos(mac, binary, root / "unused.pkg", "1.2.3", "aarch64")
            assert all("--scripts" not in call.args for call in run.call_args_list)
        agent = plistlib.loads((mac / "Library/LaunchAgents/com.thevelasquez.v_tileworkizer.plist").read_bytes())
        assert agent["ProgramArguments"] == ["/Applications/v_tileworkizer.app/Contents/MacOS/v_tileworkizer", "--service"]
        assert agent["LimitLoadToSessionType"] == "Aqua"
        app = plistlib.loads((mac / "Applications/v_tileworkizer.app/Contents/Info.plist").read_bytes())
        assert app["CFBundleVersion"] == "1.2.3"
        distribution = ET.parse(root / "distribution.xml").getroot()
        assert distribution.find("domains").get("enable_currentUserHome") == "false"
        windows = root / "windows-payload"
        windows.mkdir()
        with patch.dict(os.environ, {}, clear=True), patch.object(package_desktop, "run") as run:
            package_desktop.windows(windows, binary, root / "unused.msi", "1.2.3", "x86_64")
            assert run.call_args.args[0:2] == ("wix", "build")
        ns = {"w": "http://wixtoolset.org/schemas/v4/wxs"}
        authoring = ET.parse(root / "app.wxs").getroot()
        assert authoring.find("w:Package", ns).get("Scope") == "perMachine"
        assert not authoring.findall(".//w:CustomAction", ns)
        registry = authoring.findall(".//w:RegistryValue", ns)
        assert any(r.get("Root") == "HKLM" and "--supervise" in r.get("Value", "") for r in registry)
        for element in authoring.findall(".//w:File", ns):
            assert Path(element.get("Source")).exists()
        for stage in [deb, mac, windows]:
            assert not any(p.suffix in {".sh", ".ps1", ".py", ".bat", ".cmd"} for p in stage.rglob("*") if p.is_file())


if __name__ == "__main__":
    check()
    print("Installer authoring checks passed; no compiler or packaging tool was executed.")
