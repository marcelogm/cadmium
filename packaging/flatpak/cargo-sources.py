#!/usr/bin/env python3
"""Writes the flatpak-builder sources that vendor every crates.io crate in Cargo.lock.

The Flatpak build runs offline, so each crate is downloaded by flatpak-builder and cargo
is pointed at the vendor directory. Usage: cargo-sources.py Cargo.lock > cargo-sources.json
"""

import json
import sys
import tomllib

CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"
VENDOR = "cargo/vendor"
CARGO_CONFIG = f"""[source.vendored-sources]
directory = "{VENDOR}"

[source.crates-io]
replace-with = "vendored-sources"
"""


def crate_sources(package):
    name, version, checksum = package["name"], package["version"], package["checksum"]
    dest = f"{VENDOR}/{name}-{version}"
    return [
        {
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
            "sha256": checksum,
            "dest": dest,
        },
        {
            "type": "inline",
            "contents": json.dumps({"package": checksum, "files": {}}),
            "dest": dest,
            "dest-filename": ".cargo-checksum.json",
        },
    ]


def main():
    with open(sys.argv[1], "rb") as lock_file:
        packages = tomllib.load(lock_file)["package"]
    unsupported = [p["name"] for p in packages if p.get("source", CRATES_IO) != CRATES_IO]
    if unsupported:
        sys.exit(f"only crates.io dependencies are supported: {', '.join(unsupported)}")
    sources = [s for p in packages if "source" in p for s in crate_sources(p)]
    sources.append(
        {"type": "inline", "contents": CARGO_CONFIG, "dest": "cargo", "dest-filename": "config.toml"}
    )
    json.dump(sources, sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
