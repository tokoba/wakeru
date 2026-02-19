#!/bin/bash
# Workspace-wide code coverage measurement using cargo-llvm-cov with nextest integration
# Supports both nextest and regular test runs, with doctest merging capability

set -euo pipefail

echo "========================================="
echo "Workspace Code Coverage Measurement (nextest + llvm-cov)"
echo "========================================="

# Determine workspace root (directory of this script)
# Fallback when BASH_SOURCE is undefined (Windows/MSYS environments)
if [ -n "${BASH_SOURCE[0]:-}" ]; then
    WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
else
    # Fallback for environments like Windows/MSYS
    WORKSPACE_ROOT="$(cd "$(dirname "$0")" && pwd)"
fi
cd "$WORKSPACE_ROOT"

# Tool checks
if ! command -v cargo >/dev/null 2>&1; then
  echo "❌ cargo not found. Please install the Rust toolchain."
  exit 1
fi
if ! cargo llvm-cov --version >/dev/null 2>&1; then
  echo "❌ cargo-llvm-cov not found. Install it with:"
  echo "   cargo install cargo-llvm-cov"
  exit 1
fi

# Check if nextest is available (optional but recommended)
USE_NEXTEST=false
if cargo nextest --version >/dev/null 2>&1; then
  USE_NEXTEST=true
  echo "✅ cargo-nextest detected. Collecting coverage using nextest."
else
  echo "⚠️ cargo-nextest not found. Falling back to regular cargo test."
  echo "   To use nextest: cargo install cargo-nextest"
fi

# Parse command line arguments
INCLUDE_DOCTESTS=false
OUTPUT_FORMAT="both" # html, lcov, both

for arg in "$@"; do
  case $arg in
    --with-doctests)
      INCLUDE_DOCTESTS=true
      echo "📚 Collecting coverage including doctests"
      ;;
    --html-only)
      OUTPUT_FORMAT="html"
      echo "📄 Generating HTML report only"
      ;;
    --lcov-only)
      OUTPUT_FORMAT="lcov"
      echo "📊 Generating LCOV report only"
      ;;
    --help|-h)
      echo "Usage: $0 [options]"
      echo "Options:"
      echo "  --with-doctests  Collect coverage including doctests"
      echo "  --html-only      Generate HTML report only"
      echo "  --lcov-only      Generate LCOV report only"
      echo "  --help, -h       Show this help"
      exit 0
      ;;
  esac
done

# Coverage stability settings (keep existing behavior)
echo "Configuring environment variables for stable coverage..."
export RUST_LOG="${RUST_LOG:-warn}"
export PROPTEST_CASES="${PROPTEST_CASES:-32}"
export PROPTEST_MAX_SHRINK_ITERS="${PROPTEST_MAX_SHRINK_ITERS:-8}"
export PROPTEST_MAX_SHRINK_TIME="${PROPTEST_MAX_SHRINK_TIME:-0}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-0}"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-1}"

echo "Environment variables configured:"
echo "  RUST_LOG=$RUST_LOG"
echo "  PROPTEST_CASES=$PROPTEST_CASES"
echo "  PROPTEST_MAX_SHRINK_ITERS=$PROPTEST_MAX_SHRINK_ITERS"
echo "  PROPTEST_MAX_SHRINK_TIME=$PROPTEST_MAX_SHRINK_TIME"
echo "  RUST_BACKTRACE=$RUST_BACKTRACE"
echo "  RUST_TEST_THREADS=$RUST_TEST_THREADS"

# Dynamically detect workspace members under crates/, excluding _template-*
CRATES_DIR="$WORKSPACE_ROOT/crates"
CRATES=()
if [ -d "$CRATES_DIR" ]; then
  for d in "$CRATES_DIR"/*; do
    [ -d "$d" ] || continue
    base="$(basename "$d")"
    case "$base" in _template-*) continue ;; esac
    [ -f "$d/Cargo.toml" ] || continue
    CRATES+=("$base")
  done
fi
CRATE_COUNT="${#CRATES[@]}"
if [ "$CRATE_COUNT" -gt 0 ]; then
  echo ""
  echo "Detected workspace members (${CRATE_COUNT}): ${CRATES[*]}"
else
  echo ""
  echo "⚠️ No valid crates were detected under crates/."
  echo "   Please check your Cargo.toml [workspace] configuration (members/exclude)."
fi

# Rough runtime estimate (per crate)
# Heuristic: ~20–40 seconds per crate (first run may be longer due to build)
MIN_SEC=$((CRATE_COUNT * 20))
MAX_SEC=$((CRATE_COUNT * 40))
if [ "$CRATE_COUNT" -eq 0 ]; then
  # Even with 0 detected, tests may still run for root or other members.
  MIN_SEC=20
  MAX_SEC=40
fi
MIN_MIN=$(( (MIN_SEC + 59) / 60 ))
MAX_MIN=$(( (MAX_SEC + 59) / 60 ))

echo ""
echo "🕒 Estimated runtime: about ${MIN_SEC}–${MAX_SEC} seconds (≈ ${MIN_MIN}–${MAX_MIN} minutes)"
echo "   Note: the first run may take longer because it includes building."

# Prepare output paths
LCOV_OUT="$WORKSPACE_ROOT/target/coverage/lcov.info"
HTML_SRC="$WORKSPACE_ROOT/target/llvm-cov/html"
HTML_DST="$WORKSPACE_ROOT/target/coverage/html"
mkdir -p "$(dirname "$LCOV_OUT")"

START_TS=$(date +%s)

echo ""
echo "[1/3] Cleaning previous coverage data..."
cargo llvm-cov clean --workspace

echo ""
echo "[2/3] Running tests with coverage for entire workspace..."

if [ "$USE_NEXTEST" = true ]; then
  if [ "$INCLUDE_DOCTESTS" = true ]; then
    echo "Running nextest with coverage (excluding doctests)..."
    cargo llvm-cov --no-report nextest --workspace --all-features
    echo "Running doctests with coverage..."
    cargo llvm-cov --no-report --doc --workspace --all-features
    echo "Generating merged coverage reports..."

    if [ "$OUTPUT_FORMAT" = "lcov" ] || [ "$OUTPUT_FORMAT" = "both" ]; then
      echo "Generating LCOV coverage report..."
      cargo llvm-cov report --doctests --lcov --output-path "$LCOV_OUT"
    fi

    if [ "$OUTPUT_FORMAT" = "html" ] || [ "$OUTPUT_FORMAT" = "both" ]; then
      echo "Generating HTML coverage report..."
      cargo llvm-cov report --doctests --html
    fi
  else
    echo "Running nextest with coverage..."
    if [ "$OUTPUT_FORMAT" = "lcov" ] || [ "$OUTPUT_FORMAT" = "both" ]; then
      echo "Generating LCOV coverage report..."
      cargo llvm-cov nextest --workspace --all-features --lcov --output-path "$LCOV_OUT"
    fi

    if [ "$OUTPUT_FORMAT" = "html" ] || [ "$OUTPUT_FORMAT" = "both" ]; then
      echo "Generating HTML coverage report..."
      cargo llvm-cov nextest --workspace --all-features --html
    fi
  fi
else
  # Fallback to regular cargo test
  if [ "$INCLUDE_DOCTESTS" = true ]; then
    echo "Running cargo test with coverage (including doctests)..."
    TEST_FLAGS="--workspace --all-features --doc"
  else
    echo "Running cargo test with coverage (excluding doctests)..."
    TEST_FLAGS="--workspace --all-features"
  fi

  if [ "$OUTPUT_FORMAT" = "lcov" ] || [ "$OUTPUT_FORMAT" = "both" ]; then
    echo "Generating LCOV coverage report..."
    cargo llvm-cov $TEST_FLAGS --lcov --output-path "$LCOV_OUT"
  fi

  if [ "$OUTPUT_FORMAT" = "html" ] || [ "$OUTPUT_FORMAT" = "both" ]; then
    echo "Generating HTML coverage report..."
    cargo llvm-cov $TEST_FLAGS --html
  fi
fi

echo ""
echo "[3/3] Preparing reports..."
# Copy HTML to a simpler path for discoverability
if [ -d "$HTML_SRC" ]; then
  rm -rf "$HTML_DST"
  mkdir -p "$HTML_DST"
  cp -r "$HTML_SRC"/* "$HTML_DST/"
fi

END_TS=$(date +%s)
ELAPSED=$((END_TS - START_TS))
ELAPSED_MIN=$((ELAPSED / 60))
ELAPSED_SEC=$((ELAPSED % 60))

echo ""
echo "========================================="
echo "Coverage Summary (workspace):"
echo "========================================="
# Print summary without re-running tests:
# Prefer 'report' subcommand if available, otherwise fallback to summary-only
if cargo llvm-cov --help 2>/dev/null | grep -q "^\s*report\b"; then
  cargo llvm-cov report
else
  cargo llvm-cov --workspace --summary-only
fi

echo ""
echo "========================================="
echo "Coverage report generated!"
echo "HTML report (original): target/llvm-cov/html/index.html"
echo "HTML report (copied)  : target/coverage/html/index.html"
echo "LCOV output           : target/coverage/lcov.info"
echo "Elapsed time          : ${ELAPSED_MIN}m ${ELAPSED_SEC}s"
echo "========================================="
