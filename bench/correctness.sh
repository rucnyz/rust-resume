#!/usr/bin/env bash
# Correctness test: compare fr (Python) vs ase (Rust) session parsing
set -uo pipefail

FR_RS="${BASH_SOURCE[0]%/*}/../target/release/ase"
FR_PY_DIR="$HOME/src/fast-resume"
PASS=0
FAIL=0

result() {
    if [[ "$1" == "pass" ]]; then
        echo "PASS: $2"
        ((PASS++))
    else
        echo "FAIL: $2"
        ((FAIL++))
    fi
}

echo "=== Correctness: fr (Python) vs ase (Rust) ==="
echo ""

# Get Python Claude session IDs
py_ids=$(cd "$FR_PY_DIR" && uv run python3 -c "
from fast_resume.search import SessionSearch
s = SessionSearch()
sessions = s.get_all_sessions()
for x in sorted(sessions, key=lambda x: x.id):
    if x.agent == 'claude':
        print(x.id)
" 2>/dev/null)

# Get Rust Claude session IDs
rs_ids=$("$FR_RS" --ids -a claude 2>/dev/null | sort)

py_count=$(echo "$py_ids" | grep -c . || true)
rs_count=$(echo "$rs_ids" | grep -c . || true)
common=$(comm -12 <(echo "$py_ids") <(echo "$rs_ids") | grep -c . || true)
py_only=$(comm -23 <(echo "$py_ids") <(echo "$rs_ids") | grep -c . || true)
rs_only=$(comm -13 <(echo "$py_ids") <(echo "$rs_ids") | grep -c . || true)

echo "Claude sessions: Python=$py_count, Rust=$rs_count"
echo "Common: $common, Python-only: $py_only, Rust-only: $rs_only"

if [[ "$py_count" -gt 0 && "$rs_count" -gt 0 ]]; then
    max=$((py_count > rs_count ? py_count : rs_count))
    overlap_pct=$((common * 100 / max))
    echo "Overlap: ${overlap_pct}%"

    if [[ "$overlap_pct" -ge 95 ]]; then
        result pass "Session IDs: ${overlap_pct}% overlap ($common/$max)"
    elif [[ "$overlap_pct" -ge 80 ]]; then
        result pass "Session IDs: ${overlap_pct}% overlap (acceptable, minor parsing diffs)"
    else
        result fail "Session IDs: only ${overlap_pct}% overlap"
        echo ""
        echo "  Python-only (first 5):"
        comm -23 <(echo "$py_ids") <(echo "$rs_ids") | head -5 | sed 's/^/    /'
        echo "  Rust-only (first 5):"
        comm -13 <(echo "$py_ids") <(echo "$rs_ids") | head -5 | sed 's/^/    /'
    fi
else
    result fail "One side has 0 sessions"
fi

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
