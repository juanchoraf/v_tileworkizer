"""Declarative macOS PKG and Windows MSI authoring. No installer custom scripts."""
import os
from pathlib import Path
import plistlib
import shutil
import xml.etree.ElementTree as ET
from package import ROOT, copy, run, write


def macos(stage, binary, output, version, arch):
    app = stage / "Applications/v_tileworkizer.app/Contents"
    copy(binary, app / "MacOS/v_tileworkizer")
    (app / "MacOS/v_tileworkizer").chmod(0o755)
    copy(ROOT / "assets/logo.icns", app / "Resources/logo.icns")
    for name in ["LICENSE-MIT", "LICENSE-APACHE"]:
        copy(ROOT / name, app / "Resources" / name)
    info = {"CFBundleIdentifier": "com.thevelasquez.v_tileworkizer", "CFBundleName": "v_tileworkizer",
        "CFBundleExecutable": "v_tileworkizer", "CFBundlePackageType": "APPL", "CFBundleIconFile": "logo",
        "CFBundleShortVersionString": version, "CFBundleVersion": version, "NSHighResolutionCapable": True,
        "NSAccessibilityUsageDescription": "Arrange your application windows into your chosen workspace layout."}
    (app / "Info.plist").write_bytes(plistlib.dumps(info))
    agent = {"Label": "com.thevelasquez.v_tileworkizer",
        "ProgramArguments": ["/Applications/v_tileworkizer.app/Contents/MacOS/v_tileworkizer", "--service"],
        "RunAtLoad": True, "KeepAlive": {"SuccessfulExit": False}, "ThrottleInterval": 5,
        "LimitLoadToSessionType": "Aqua", "ProcessType": "Interactive"}
    agent_path = stage / "Library/LaunchAgents/com.thevelasquez.v_tileworkizer.plist"
    agent_path.parent.mkdir(parents=True)
    agent_path.write_bytes(plistlib.dumps(agent))
    identity = os.environ.get("MACOS_SIGN_IDENTITY")
    if identity:
        run("codesign", "--force", "--options", "runtime", "--timestamp", "--sign", identity, app.parent)
    else:
        run("codesign", "--force", "--sign", "-", app.parent)
    component = stage.parent / "component.pkg"
    run("pkgbuild", "--root", stage, "--ownership", "recommended", "--identifier",
        "com.thevelasquez.v_tileworkizer", "--version", version, "--install-location", "/", component)
    resources = stage.parent / "resources"
    resources.mkdir()
    copy(ROOT / "assets/logo.png", resources / "logo.png")
    write(resources / "welcome.html", '<html><body><img src="logo.png" width="96" height="96">'
        '<h1>v_tileworkizer</h1><p>A place for every window.</p>'
        '<p>Installs for every user. The background service starts at the next login.</p>'
        '<p>Open the app and grant Accessibility access to enable window arrangement.</p></body></html>')
    copy(ROOT / "LICENSE-MIT", resources / "license.txt")
    distribution = ET.Element("installer-gui-script", {"minSpecVersion": "2"})
    ET.SubElement(distribution, "title").text = "v_tileworkizer"
    ET.SubElement(distribution, "welcome", {"file": "welcome.html"})
    ET.SubElement(distribution, "license", {"file": "license.txt"})
    ET.SubElement(distribution, "options", {"customize": "never", "require-scripts": "false", "rootVolumeOnly": "true"})
    ET.SubElement(distribution, "domains", {"enable_localSystem": "true", "enable_currentUserHome": "false", "enable_anywhere": "false"})
    outline = ET.SubElement(distribution, "choices-outline")
    ET.SubElement(outline, "line", {"choice": "app"})
    choice = ET.SubElement(distribution, "choice", {"id": "app", "visible": "false", "title": "v_tileworkizer"})
    ET.SubElement(choice, "pkg-ref", {"id": "com.thevelasquez.v_tileworkizer"})
    ET.SubElement(distribution, "pkg-ref", {"id": "com.thevelasquez.v_tileworkizer", "version": version}).text = "component.pkg"
    path = stage.parent / "distribution.xml"
    ET.ElementTree(distribution).write(path, encoding="utf-8", xml_declaration=True)
    args = ["productbuild", "--distribution", path, "--package-path", stage.parent, "--resources", resources]
    if os.environ.get("MACOS_INSTALLER_IDENTITY"):
        args += ["--sign", os.environ["MACOS_INSTALLER_IDENTITY"]]
    run(*args, output)
    if os.environ.get("MACOS_NOTARY_PROFILE"):
        run("xcrun", "notarytool", "submit", output, "--keychain-profile", os.environ["MACOS_NOTARY_PROFILE"], "--wait")
        run("xcrun", "stapler", "staple", output)


def windows(stage, binary, output, version, arch):
    wix_arch = {"x86_64": "x64", "aarch64": "arm64"}[arch]
    namespace = "http://wixtoolset.org/schemas/v4/wxs"
    ui_namespace = "http://wixtoolset.org/schemas/v4/wxs/ui"
    ET.register_namespace("", namespace)
    ET.register_namespace("ui", ui_namespace)
    def node(parent, tag, **attrs):
        return ET.SubElement(parent, "{" + namespace + "}" + tag, attrs)
    root = ET.Element("{" + namespace + "}Wix")
    package = node(root, "Package", Name="v_tileworkizer", Manufacturer="TheVelasquez",
        Version=version, UpgradeCode="FA6E87CB-7803-4A03-A04D-8819A98402B9", Scope="perMachine", Language="1033")
    node(package, "MajorUpgrade", DowngradeErrorMessage="A newer v_tileworkizer is already installed.")
    node(package, "MediaTemplate", EmbedCab="yes")
    node(package, "Icon", Id="Logo", SourceFile=str(ROOT / "assets/logo.ico"))
    node(package, "Property", Id="ARPPRODUCTICON", Value="Logo")
    node(package, "Property", Id="ARPURLINFOABOUT", Value="https://github.com/juanchoraf/v_tileworkizer")
    program_files = node(package, "StandardDirectory", Id="ProgramFiles64Folder")
    install = node(program_files, "Directory", Id="INSTALLFOLDER", Name="v_tileworkizer")
    component = node(install, "Component", Id="Application", Guid="*")
    node(component, "File", Id="AppExe", Source=str(binary), KeyPath="yes")
    for index, name in enumerate(["LICENSE-MIT", "LICENSE-APACHE"]):
        node(component, "File", Id=f"License{index}", Source=str(ROOT / name))
    node(component, "RegistryValue", Root="HKLM", Key="Software\\Microsoft\\Windows\\CurrentVersion\\Run",
        Name="v_tileworkizer", Type="string", Value='"[INSTALLFOLDER]v_tileworkizer.exe" --supervise')
    programs = node(package, "StandardDirectory", Id="ProgramMenuFolder")
    shortcut = node(programs, "Component", Id="StartMenu", Guid="*")
    node(shortcut, "Shortcut", Id="OpenApp", Name="v_tileworkizer", Icon="Logo",
        Target="[#AppExe]", Arguments="--gui",
        WorkingDirectory="INSTALLFOLDER")
    node(shortcut, "RegistryValue", Root="HKLM", Key="Software\\TheVelasquez\\v_tileworkizer",
        Name="Installed", Type="integer", Value="1", KeyPath="yes")
    feature = node(package, "Feature", Id="Main", Title="v_tileworkizer", Level="1")
    node(feature, "ComponentRef", Id="Application")
    node(feature, "ComponentRef", Id="StartMenu")
    ET.SubElement(package, "{" + ui_namespace + "}WixUI", {"Id": "WixUI_InstallDir", "InstallDirectory": "INSTALLFOLDER"})
    for key, path in [("WixUIBannerBmp", "banner.bmp"), ("WixUIDialogBmp", "dialog.bmp")]:
        node(package, "WixVariable", Id=key, Value=str(ROOT / "assets" / path))
    license_text = (ROOT / "LICENSE-MIT").read_text().replace("\\", "\\\\").replace("{", "\\{").replace("}", "\\}").replace("\n", "\\par\n")
    license_path = stage.parent / "license.rtf"
    write(license_path, "{\\rtf1\\ansi " + license_text + "}")
    node(package, "WixVariable", Id="WixUILicenseRtf", Value=str(license_path))
    source = stage.parent / "app.wxs"
    ET.ElementTree(root).write(source, encoding="utf-8", xml_declaration=True)
    run("wix", "build", "-arch", wix_arch, "-ext", "WixToolset.UI.wixext", source, "-o", output)
    if os.environ.get("WINDOWS_SIGN_SHA1"):
        run("signtool", "sign", "/sha1", os.environ["WINDOWS_SIGN_SHA1"], "/fd", "SHA256",
            "/tr", os.environ.get("WINDOWS_TIMESTAMP_URL", "http://timestamp.digicert.com"), "/td", "SHA256", output)
