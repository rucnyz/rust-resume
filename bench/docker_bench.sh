#!/usr/bin/env bash
# Benchmark inside Docker container.
# Expects Claude data mounted at ~/.claude/projects
# and binaries at ~/.local/bin/{ase,cass,cc-sessions,ccrider,ccsearch}
set -euo pipefail

SESSION_COUNT=$(find ~/.claude/projects -name "*.jsonl" 2>/dev/null | wc -l)
echo "========================================="
echo "  Docker Benchmark ($SESSION_COUNT sessions)"
echo "========================================="
echo ""

ASE=ase
CASS=cass
CC_SESSIONS=cc-sessions
CCRIDER=ccrider
CCSEARCH=ccsearch

# Check which tools are available
TOOLS=()
command -v $ASE &>/dev/null && TOOLS+=("ase")
command -v $CASS &>/dev/null && TOOLS+=("cass")
command -v $CC_SESSIONS &>/dev/null && TOOLS+=("cc-sessions")
command -v $CCRIDER &>/dev/null && TOOLS+=("ccrider")
command -v $CCSEARCH &>/dev/null && TOOLS+=("ccsearch")
echo "Available tools: ${TOOLS[*]}"
echo ""

# --- Build indexes first ---
echo "=== Building indexes ==="
echo -n "ase --rebuild: "
$ASE --rebuild --list >/dev/null 2>&1 && echo "ok" || echo "fail"

if command -v $CASS &>/dev/null; then
    echo -n "cass index: "
    $CASS index >/dev/null 2>&1 && echo "ok" || echo "fail"
fi

if command -v $CCRIDER &>/dev/null; then
    echo -n "ccrider sync: "
    $CCRIDER sync >/dev/null 2>&1 && echo "ok" || echo "fail"
fi

# ccsearch and cc-sessions build index on first run
if command -v $CCSEARCH &>/dev/null; then
    echo -n "ccsearch warmup: "
    $CCSEARCH list >/dev/null 2>&1 && echo "ok" || echo "fail"
fi
if command -v $CC_SESSIONS &>/dev/null; then
    echo -n "cc-sessions warmup: "
    $CC_SESSIONS --list >/dev/null 2>&1 && echo "ok" || echo "fail"
fi
echo ""

# --- List benchmark ---
echo "=== 1. List sessions (warm, Claude only) ==="
ARGS=(-n "ase" "$ASE --list --agent claude 2>/dev/null")
command -v $CC_SESSIONS &>/dev/null && ARGS+=(-n "cc-sessions" "$CC_SESSIONS --list 2>/dev/null")
command -v $CCRIDER &>/dev/null && ARGS+=(-n "ccrider" "$CCRIDER list 2>/dev/null")
command -v $CCSEARCH &>/dev/null && ARGS+=(-n "ccsearch" "$CCSEARCH list 2>/dev/null")
hyperfine --warmup 2 --min-runs 10 "${ARGS[@]}"

echo ""
echo "=== 2. Search 'niri' (Claude only) ==="
ARGS=(-n "ase" "$ASE --list --agent claude 'niri' 2>/dev/null")
command -v $CASS &>/dev/null && ARGS+=(-n "cass" "$CASS search 'niri' --agent claude-code --robot 2>/dev/null")
command -v $CCRIDER &>/dev/null && ARGS+=(-n "ccrider" "$CCRIDER search 'niri' 2>/dev/null")
command -v $CCSEARCH &>/dev/null && ARGS+=(-n "ccsearch" "$CCSEARCH search 'niri' --no-tui 2>/dev/null")
hyperfine --warmup 2 --min-runs 10 "${ARGS[@]}"

echo ""
echo "=== 3. Cold rebuild ==="
# Only test ase and ccrider (others are too slow or don't have rebuild)
ARGS=()
ARGS+=(-n "ase" --prepare "rm -rf ~/.cache/agents-sesame/tantivy_index_rs" "$ASE --rebuild --list --agent claude 2>/dev/null")
command -v $CCRIDER &>/dev/null && ARGS+=(-n "ccrider" --prepare "rm -f ~/.local/share/ccrider/ccrider.db" "$CCRIDER sync 2>/dev/null && $CCRIDER search 'test' 2>/dev/null")
hyperfine --warmup 0 --min-runs 3 "${ARGS[@]}"

echo ""
echo "=== 4. Binary sizes ==="
for tool in $ASE $CASS $CC_SESSIONS $CCRIDER $CCSEARCH; do
    p=$(which "$tool" 2>/dev/null) && ls -lh "$p" | awk -v name="$tool" '{print name":", $5}' || true
done

echo ""
echo "=== Summary ==="
echo "Sessions: $SESSION_COUNT"
echo "Tools tested: ${TOOLS[*]}"
