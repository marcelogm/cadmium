#!/usr/bin/env bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

limit=400
allowlist=.github/oversized-files.txt
status=0

declare -A recorded
if [[ -f $allowlist ]]; then
    while read -r lines path; do
        [[ -n ${path:-} ]] && recorded[$path]=$lines
    done < "$allowlist"
fi

fail() {
    echo "$1"
    status=1
}

while IFS= read -r path; do
    lines=$(wc -l < "$path")
    if [[ -v recorded[$path] ]]; then
        allowed=${recorded[$path]}
        unset "recorded[$path]"
        if (( lines > allowed )); then
            fail "$path: $lines lines, grew past the $allowed recorded in $allowlist"
        elif (( lines <= limit )); then
            fail "$path: $lines lines, within $limit now; remove it from $allowlist"
        elif (( lines < allowed )); then
            fail "$path: $lines lines, lower its count in $allowlist from $allowed"
        fi
    elif (( lines > limit )); then
        fail "$path: $lines lines, over the $limit-line limit"
    fi
done < <(git ls-files --cached --others --exclude-standard -- '*.rs' '*.wgsl')

for path in "${!recorded[@]}"; do
    fail "$path: listed in $allowlist but not in the tree"
done

exit $status
