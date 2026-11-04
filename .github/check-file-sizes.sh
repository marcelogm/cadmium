#!/usr/bin/env bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

limit=400
status=0

while IFS= read -r path; do
    lines=$(wc -l < "$path")
    if (( lines > limit )); then
        echo "$path: $lines lines, over the $limit-line limit"
        status=1
    fi
done < <(git ls-files --cached --others --exclude-standard -- '*.rs' '*.wgsl')

exit $status
