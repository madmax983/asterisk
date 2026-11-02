#!/usr/bin/env bash
#
# Asterisk — the authoritative verification gate.
#
# This is the one that counts. It runs on the pinned toolchain and runs Verus
# over verus/. The gates run in the cloud during the build are a weaker
# pre-check on a different machine with no Verus at all; a green cloud build
# says nothing about the proofs.
#
# There are no services to start: the command log and the Harvest runtime are
# two SQLite files the app opens itself (docs/adr/0002). So there is no
# `docker compose` step — the end-to-end gate boots the real binary in a
# scratch data directory, drives it through the CSRF-protected endpoints the
# browser uses, kills it with SIGKILL mid-chronicle, boots it again on the
# same files, and requires that nothing logged was lost and every family
# replays exactly. Safe to run twice.
#
#   ./verify.sh          run everything
#   ./verify.sh --quick  skip the boot / end-to-end gate
#
set -euo pipefail
cd "$(dirname "$0")"

GATES=()
STATUSES=()
FAILED=0
QUICK=0
APP_PID=""
SCRATCH=""
PORT="${ASTERISK_VERIFY_PORT:-3011}"

if [ "${1:-}" = "--quick" ]; then QUICK=1; fi

if [ -t 1 ]; then
    BOLD=$(printf '\033[1m'); RED=$(printf '\033[31m'); GREEN=$(printf '\033[32m')
    YELLOW=$(printf '\033[33m'); OFF=$(printf '\033[0m')
else
    BOLD=""; RED=""; GREEN=""; YELLOW=""; OFF=""
fi

record() { GATES+=("$1"); STATUSES+=("$2"); if [ "$2" = "FAIL" ]; then FAILED=1; fi; }
step() { printf '\n%s── %s %s\n' "$BOLD" "$1" "$OFF"; }
run_gate() {
    local name="$1"; shift
    step "$name"
    if "$@"; then record "$name" "PASS"; else record "$name" "FAIL"; fi
}
cleanup() {
    if [ -n "${APP_PID:-}" ]; then kill "$APP_PID" 2>/dev/null || true; wait "$APP_PID" 2>/dev/null || true; fi
    if [ -n "${SCRATCH:-}" ] && [ -d "$SCRATCH" ]; then rm -rf "$SCRATCH"; fi
}
trap cleanup EXIT INT TERM

# ── Gate 0: the toolchain must match rust-toolchain.toml ────────────────────
step "toolchain"
EXPECTED=$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' rust-toolchain.toml | head -1)
if [ -z "$EXPECTED" ]; then printf '%sFAIL%s cannot read channel from rust-toolchain.toml\n' "$RED" "$OFF"; exit 1; fi
if ! command -v cargo >/dev/null 2>&1; then printf '%sFAIL%s cargo is not on PATH\n' "$RED" "$OFF"; exit 1; fi
ACTUAL=$(cargo --version | awk '{print $2}')
printf 'expected %s, found cargo %s\n' "$EXPECTED" "$ACTUAL"
if [ "$ACTUAL" != "$EXPECTED" ]; then
    printf '%sFAIL%s toolchain mismatch: rust-toolchain.toml pins %s, cargo reports %s.\n' "$RED" "$OFF" "$EXPECTED" "$ACTUAL"
    printf '  Install it with:  rustup toolchain install %s\n' "$EXPECTED"
    printf '  Refusing to run: a green result on the wrong compiler is worse than none.\n'
    exit 1
fi
record "toolchain" "PASS"

# ── Gates 1-3: format, lint, tests ──────────────────────────────────────────
run_gate "cargo fmt"    cargo fmt --all -- --check
run_gate "cargo clippy" cargo clippy --all-targets -- -D warnings
run_gate "cargo test"   cargo test --workspace

# ── Gate 4: services ────────────────────────────────────────────────────────
step "services"
printf 'none: two embedded SQLite files (docs/adr/0002-durable-replay-on-harvest.md)\n'
record "services (none)" "PASS"

# ── Gate 5: boot and end to end ─────────────────────────────────────────────
# `curl | grep -q` would trip pipefail when grep exits early, so fetch to a
# file first and grep the file.
serves() {
    curl -fsS "$@" -o "$SCRATCH/body" && grep -q -- "$PATTERN" "$SCRATCH/body"
}

boot_app() {
    AUTUMN_PROFILE=dev AUTUMN_SERVER__PORT="$PORT" ASTERISK_DATA="$SCRATCH/data" ASTERISK_CENTURY_SECS=1 \
    AUTUMN_SECURITY__SIGNING_SECRET="${AUTUMN_SECURITY__SIGNING_SECRET:-verify-sh-secret-not-for-production-0123456789}" \
        ./target/release/asterisk-app >> "$SCRATCH/boot.log" 2>&1 &
    APP_PID=$!
    local base="http://127.0.0.1:$PORT"
    for _ in $(seq 1 60); do
        if ! kill -0 "$APP_PID" 2>/dev/null; then break; fi
        if curl -fsS "$base/health" >/dev/null 2>&1; then return 0; fi
        sleep 1
    done
    printf 'did not boot:\n'; tail -40 "$SCRATCH/boot.log"; return 1
}

# A number out of a JSON body: `num commands body.json`.
num() { sed -n "s/.*\"$1\":\([0-9][0-9]*\).*/\1/p" "$2" | head -1; }

e2e() {
    SCRATCH=$(mktemp -d "${TMPDIR:-/tmp}/asterisk-verify.XXXXXX")
    local base="http://127.0.0.1:$PORT" jar="$SCRATCH/jar" fam="/f/the-asterisk-family"
    if curl -fsS --max-time 2 "$base/health" >/dev/null 2>&1; then
        printf '%sFAIL%s something already serves on port %s; set ASTERISK_VERIFY_PORT\n' "$RED" "$OFF" "$PORT"
        return 1
    fi
    cargo build --release --bin asterisk-app
    boot_app || return 1

    curl -fsS "$base$fam" -c "$jar" -b "$jar" -o "$SCRATCH/family.html"
    grep -q 'svg class="tree"' "$SCRATCH/family.html" || { echo "no tree on $fam"; return 1; }
    PATTERN='The Asterisk family' serves "$base/" || { echo 'the gallery'; return 1; }
    PATTERN='Grow a family' serves "$base/new" || { echo '/new'; return 1; }
    PATTERN='Its cognates' serves "$base$fam/lang/1/word/moon" || { echo 'an etymology'; return 1; }
    PATTERN='Writing a word list' serves "$base/about" || { echo '/about'; return 1; }
    PATTERN='window.asterisk' serves "$base/asterisk.js" || { echo '/asterisk.js'; return 1; }
    PATTERN='"replays":true' serves "$base$fam/_world.json" || { echo 'the first family does not replay'; return 1; }

    local token
    token=$(sed -n 's/.*name="csrf-token" content="\([^"]*\)".*/\1/p' "$SCRATCH/family.html" | head -1)
    [ -n "$token" ] || { echo 'no csrf token in the page'; return 1; }

    # Without the token: refused.
    local code
    code=$(curl -s -o /dev/null -w '%{http_code}' -b "$jar" -X POST "$base$fam/apply" --data-urlencode 'rule=p > f' -d 'target=all')
    [ "$code" = "403" ] || { echo "an apply without a CSRF token got $code, wanted 403"; return 1; }
    # With it: the rule goes to the chronicle.
    PATTERN='sent to the chronicle' serves -b "$jar" -c "$jar" -H "x-csrf-token: $token" -X POST "$base$fam/apply" \
        --data-urlencode 'rule=V > [+nasal] / _N' -d 'target=all&hue=200' || { echo 'the apply did not land'; return 1; }
    # A family from a word list.
    PATTERN='"slug":"verify-family"' serves -b "$jar" -c "$jar" -H "x-csrf-token: $token" -X POST "$base/families" \
        -d 'title=Verify family&seed=verify&span=100&century_secs=1' \
        --data-urlencode $'vocab=moon = mena\nstar\nPL = s\nstar.PL' || { echo 'a family did not grow'; return 1; }
    PATTERN='"meanings":6' serves "$base/f/verify-family/_world.json" || { echo 'the word list was not used'; return 1; }

    # The Harvest clock runs: centuries and the hand rule join the log.
    sleep 4
    curl -fsS "$base$fam/_world.json" -o "$SCRATCH/before.json"
    local before
    before=$(num commands "$SCRATCH/before.json")
    [ "${before:-0}" -ge 2 ] || { echo "the clock logged ${before:-0} commands in 4 s"; return 1; }
    PATTERN='by hand' serves "$base$fam/_feed" || { echo 'the hand rule never reached the chronicle'; return 1; }
    PATTERN='```mermaid' serves "$base$fam/export.md" || { echo 'the Markdown export'; return 1; }
    PATTERN='"format": "asterisk-family/1"' serves "$base$fam/export.json" || { echo 'the JSON export'; return 1; }

    # Durability: kill -9 mid-chronicle, boot again on the same files.
    kill -9 "$APP_PID"; wait "$APP_PID" 2>/dev/null || true; APP_PID=""
    boot_app || return 1
    curl -fsS "$base$fam/_world.json" -o "$SCRATCH/after.json"
    local after
    after=$(num commands "$SCRATCH/after.json")
    [ "${after:-0}" -ge "$before" ] || { echo "the log lost commands across kill -9: $before then ${after:-0}"; return 1; }
    grep -q '"replays":true' "$SCRATCH/after.json" || { echo 'the rebuilt family does not replay'; return 1; }
    PATTERN='"meanings":6' serves "$base/f/verify-family/_world.json" || { echo 'the second family was lost'; return 1; }
    sleep 3
    curl -fsS "$base$fam/_world.json" -o "$SCRATCH/later.json"
    [ "$(num commands "$SCRATCH/later.json")" -gt "$after" ] || { echo 'the chronicle did not resume after the restart'; return 1; }
    printf '%sbooted; every page served; a tokenless apply was refused and one with the token reached the chronicle;\n' "$GREEN"
    printf 'a family grew from a word list; the Harvest clock logged %s commands; after kill -9 the log held %s,\n' "$before" "$after"
    printf 'every family replayed exactly, and the chronicle resumed%s\n' "$OFF"
}
if [ "$QUICK" -eq 1 ]; then
    step "boot + end to end"; printf '%sskipped%s (--quick)\n' "$YELLOW" "$OFF"; record "boot + end to end" "SKIP"
else
    run_gate "boot + end to end" e2e
    if [ -n "$APP_PID" ]; then kill "$APP_PID" 2>/dev/null || true; wait "$APP_PID" 2>/dev/null || true; APP_PID=""; fi
fi

# ── Gate 6: Verus ───────────────────────────────────────────────────────────
step "verus"
VERUS=""
for candidate in "$(command -v verus 2>/dev/null || true)" "${VERUS_BIN:-}" \
                 "$HOME/verus/verus" "$HOME/verus/source/target-verus/release/verus"; do
    [ -z "$candidate" ] && continue
    if [ -x "$candidate" ]; then VERUS="$candidate"; break; fi
done
if [ -z "$VERUS" ]; then
    printf '%sFAIL%s no Verus binary found. Tried, in order:\n' "$RED" "$OFF"
    printf '    verus on PATH\n    $VERUS_BIN (%s)\n    %s/verus/verus\n    %s/verus/source/target-verus/release/verus\n' \
        "${VERUS_BIN:-unset}" "$HOME" "$HOME"
    record "verus" "FAIL"
else
    printf 'using %s\n' "$VERUS"
    for source in verus/src/*.rs; do
        name="verus $(basename "$source" .rs)"
        if "$VERUS" "$source"; then record "$name" "PASS"
        else
            printf '  %snot verified%s — docs/SPEC.md section 5 ranks the obligations by confidence.\n' "$RED" "$OFF"
            record "$name" "FAIL"
        fi
    done
fi

# ── Summary ─────────────────────────────────────────────────────────────────
printf '\n%s─────────── asterisk verify ───────────%s\n' "$BOLD" "$OFF"
i=0
while [ "$i" -lt "${#GATES[@]}" ]; do
    case "${STATUSES[$i]}" in PASS) c="$GREEN" ;; FAIL) c="$RED" ;; *) c="$YELLOW" ;; esac
    printf '  %-22s %s%s%s\n' "${GATES[$i]}" "$c" "${STATUSES[$i]}" "$OFF"
    i=$((i + 1))
done
printf '%s───────────────────────────────────────%s\n' "$BOLD" "$OFF"
if [ "$FAILED" -ne 0 ]; then printf '%sverification failed%s\n' "$RED" "$OFF"; exit 1; fi
printf '%sall gates passed%s\n' "$GREEN" "$OFF"
