#!/usr/bin/env python3
"""Host-only clean-extraction regression; no core, SD or device is opened."""

import hashlib
import importlib.util
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("package_test", HERE / "package-test.py")
PACKAGER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGER)


class PackageTest(unittest.TestCase):
    def test_clean_extraction_preserves_startup_directories_and_payload(self):
        payload = {
            "Apps/SlotStockTest/launch.sh": (
                HERE.parents[1] / "knulli/stock-app/launch.sh"
            ).read_bytes(),
            "Slot/System/slot": b"authored inert package fixture\x00\xff",
            "System/starts/slot.sh": b"# authored startup placeholder\n",
            "trimui/app/README.txt": b"authored dispatch placeholder\n",
        }
        for existing_logs in (False, True):
            with (
                self.subTest(existing_logs=existing_logs),
                tempfile.TemporaryDirectory() as temp,
            ):
                root = Path(temp)
                tree, extracted = root / "staging", root / "extracted"
                for name, data in payload.items():
                    path = tree / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(data)
                if existing_logs:
                    (tree / "Slot/Logs").mkdir()
                package = root / "candidate.zip"
                self.assertEqual(PACKAGER.write_package(tree, package), len(payload))
                with zipfile.ZipFile(package) as archive:
                    archive.extractall(extracted)

                self.assertEqual(
                    {
                        p.relative_to(extracted).as_posix()
                        for p in extracted.rglob("*")
                        if p.is_file()
                    },
                    set(payload) | {"MANIFEST.json", "SHA256SUMS"},
                )
                manifest = json.loads((extracted / "MANIFEST.json").read_text())
                self.assertEqual(manifest["version"], PACKAGER.VERSION)
                self.assertEqual(
                    {
                        item["path"]: (item["bytes"], item["sha256"])
                        for item in manifest["files"]
                    },
                    {
                        name: (len(data), hashlib.sha256(data).hexdigest())
                        for name, data in payload.items()
                    },
                )
                self.assertEqual(
                    (extracted / "SHA256SUMS").read_text(),
                    "".join(
                        f"{hashlib.sha256(data).hexdigest()}  {name}\n"
                        for name, data in sorted(payload.items())
                    ),
                )
                for name, data in payload.items():
                    self.assertEqual((extracted / name).read_bytes(), data)
                # These must exist before the stock launcher can start Slot;
                # root::ensure cannot repair a missing Logs directory in time.
                for directory in ("Slot", "Slot/Logs", "Slot/System"):
                    self.assertTrue((extracted / directory).is_dir(), directory)
                self.assertEqual(list((extracted / "Slot/Logs").iterdir()), [])


if __name__ == "__main__":
    unittest.main()
