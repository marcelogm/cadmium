#!/usr/bin/env python3
"""Turns a cargo-llvm-cov JSON summary into a shields.io endpoint badge.

Usage: coverage-badge.py <llvm-cov summary.json> <badge.json>
Prints the line coverage as Markdown for the job summary.
"""

import json
import pathlib
import sys

summary, badge = map(pathlib.Path, sys.argv[1:3])
lines = json.loads(summary.read_text())["data"][0]["totals"]["lines"]
percent = lines["percent"]
color = "brightgreen" if percent >= 80 else "yellow" if percent >= 60 else "orange"
badge.parent.mkdir(parents=True, exist_ok=True)
badge.write_text(
    json.dumps({"schemaVersion": 1, "label": "coverage", "message": f"{percent:.0f}%", "color": color})
)
print(f"Line coverage: **{percent:.1f}%** ({lines['covered']} of {lines['count']} lines)")
