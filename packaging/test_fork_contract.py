"""Slide's install identity stays apart from upstream DeckCraft; nothing is installed or registered.

    python3 packaging/test_fork_contract.py
"""

import json
from pathlib import Path
import plistlib
import re
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
WIX = "{http://wixtoolset.org/schemas/v4/wxs}"
# Upstream DeckCraft's MSI identity: reusing any of these would upgrade or remove its install.
UPSTREAM_GUIDS = {
    "6F9C69E7-7825-40D2-BF26-C274DA676425",  # UpgradeCode
    "D87EA444-41A4-4C19-922E-66D1A29461FF",  # DeckcraftApp component
}


def wxs():
    return ET.parse(ROOT / "packaging/windows/deckcraft.wxs").getroot()


def policy():
    return json.loads((ROOT / "packaging/windows/install-policy.json").read_text(encoding="utf-8"))


class MacBundle(unittest.TestCase):
    def test_bundle_is_slide_and_only_an_open_with_candidate(self):
        info = plistlib.loads((ROOT / "packaging/macos/Info.plist.in").read_bytes())
        self.assertEqual(info["CFBundleDisplayName"], "Slide")
        self.assertEqual(info["CFBundleName"], "Slide")
        self.assertEqual(info["CFBundleExecutable"], "Slide")
        self.assertEqual(info["CFBundleIdentifier"], "io.github.jeongeee.slide")
        self.assertTrue(all(t["LSHandlerRank"] == "Alternate" for t in info["CFBundleDocumentTypes"]))

    def test_package_names(self):
        script = (ROOT / "packaging/macos/package.sh").read_text(encoding="utf-8")
        self.assertIn('APP="$WORK/Slide.app"', script)
        self.assertIn('DMG="$DIST/slide-$VERSION-macos-$ARCH.dmg"', script)
        self.assertIn('CLI_ZIP="$DIST/slide-cli-$VERSION-macos-$ARCH.zip"', script)
        self.assertIn('-volname "Slide"', script)
        self.assertNotIn("DeckCraft.app", script)


class WindowsInstall(unittest.TestCase):
    def test_policy_has_its_own_identity_and_payloads(self):
        p = policy()
        self.assertEqual(p["id"], "slide")
        self.assertEqual(p["name"], "Slide")
        self.assertNotIn(p["upgradeCode"], UPSTREAM_GUIDS)
        self.assertEqual(sorted(p["payloads"]), ["NOTICE", "slide-cli.exe", "slide.exe"])
        for row in p["registry"]:
            self.assertNotIn("DeckCraft", row[1])
            self.assertNotIn("deckcraft.exe", row[1])

    def test_wxs_does_not_reuse_upstream_guids(self):
        root = wxs()
        package = root.find(f"{WIX}Package")
        self.assertEqual(package.attrib["Name"], "Slide")
        self.assertEqual(package.attrib["Manufacturer"], "JeongEEE")
        self.assertEqual(package.attrib["UpgradeCode"], policy()["upgradeCode"])
        guids = {node.attrib["Guid"] for node in root.iter() if "Guid" in node.attrib}
        guids.add(package.attrib["UpgradeCode"])
        self.assertFalse(guids & UPSTREAM_GUIDS)
        for g in guids:
            self.assertRegex(g, r"^[0-9A-F]{8}(-[0-9A-F]{4}){3}-[0-9A-F]{12}$")

    def test_install_folder_registry_and_progids(self):
        root = wxs()
        dirs = {d.attrib["Id"]: d.attrib["Name"] for d in root.iter(f"{WIX}Directory")}
        self.assertEqual((dirs["CraftSuiteFolder"], dirs["INSTALLFOLDER"]), ("JeongEEE", "Slide"))
        wxi = (ROOT / "packaging/windows/install-policy.wxi").read_text(encoding="utf-8")
        self.assertIn("[LocalAppDataFolder]Programs\\JeongEEE\\Slide\\", wxi)
        self.assertNotIn("DeckCraft", wxi)
        self.assertNotIn("ArtCraft", wxi)
        keys = {v.attrib["Key"] for v in root.iter(f"{WIX}RegistryValue")}
        self.assertIn("Software\\JeongEEE\\Slide", keys)
        progids = [p.attrib["Id"] for p in root.iter(f"{WIX}ProgId")]
        self.assertTrue(progids)
        self.assertTrue(all(p.startswith("JeongEEE.Slide.") for p in progids), progids)

    def test_start_menu_shortcut_name(self):
        names = [s.attrib["Name"] for s in wxs().iter(f"{WIX}Shortcut")]
        self.assertEqual(names, ["Slide (JeongEEE)"])

    def test_file_types_are_open_with_candidates_only(self):
        # An <Extension> (also under a ProgId) writes `Software\\Classes\\.<ext>` default = the ProgId.
        self.assertEqual(list(wxs().iter(f"{WIX}Extension")), [])
        # A value without a name on `Software\Classes\.<ext>` would take over the default app.
        for v in wxs().iter(f"{WIX}RegistryValue"):
            if re.fullmatch(r"Software\\Classes\\\.[^\\]+", v.attrib["Key"]):
                self.fail(f"sets the default handler of {v.attrib['Key']}")

    def test_policy_registry_matches_the_wxs(self):
        # check-install-policy.ps1 compares the compiled MSI with the policy on Windows only.
        rows = {
            (v.attrib["Key"], v.attrib.get("Name", ""), v.attrib["Value"])
            for v in wxs().iter(f"{WIX}RegistryValue")
        }
        for row in policy()["registry"]:
            self.assertIn((row[1], row[2], row[3]), rows)


if __name__ == "__main__":
    unittest.main()
