#!/bin/bash
# Enhanced nextest runner with doctest support and better options

set -euo pipefail

# Default options
INCLUDE_DOCTESTS=false
INCLUDE_IGNORED=false
RUN_DOCTESTS_ONLY=false
VERBOSE=false

# Parse command line arguments
for arg in "$@"; do
  case $arg in
    --with-doctests)
      INCLUDE_DOCTESTS=true
      echo "📚 Also running doctests"
      ;;
    --with-ignored)
      INCLUDE_IGNORED=true
      echo "🔄 Also running ignored tests"
      ;;
    --doctests-only)
      RUN_DOCTESTS_ONLY=true
      echo "📚 Running doctests only"
      ;;
    --verbose|-v)
      VERBOSE=true
      echo "🔍 Verbose output mode"
      ;;
    --help|-h)
      echo "Usage: $0 [options]"
      echo "Options:"
      echo "  --with-doctests   Run doctests in addition to regular nextest tests"
      echo "  --doctests-only   Run doctests only (skip nextest)"
      echo "  --with-ignored    Also run ignored tests"
      echo "  --verbose, -v     Verbose output"
      echo "  --help, -h        Show this help"
      echo ""
      echo "Examples:"
      echo "  $0                           # Regular nextest run"
      echo "  $0 --with-doctests           # nextest + doctest"
      echo "  $0 --doctests-only           # doctest only"
      echo "  $0 --with-ignored            # Run including ignored tests"
      exit 0
      ;;
  esac
done

echo "========================================="
echo "Running tests with nextest"
echo "========================================="

# Tool check
if ! cargo nextest --version >/dev/null 2>&1; then
  echo "❌ cargo-nextest not found. Please install it with:"
  echo "   cargo install cargo-nextest"
  exit 1
fi

# Run tests based on options
if [ "$RUN_DOCTESTS_ONLY" = true ]; then
  echo ""
  echo "📚 Running doctests only..."
  if [ "$VERBOSE" = true ]; then
    cargo test --doc --workspace --all-features -- --nocapture
  else
    cargo test --doc --workspace --all-features
  fi
else
  # Run nextest tests
  NEXTEST_CMD="cargo nextest run --all-features --all-targets"

  if [ "$INCLUDE_IGNORED" = true ]; then
    NEXTEST_CMD="$NEXTEST_CMD && cargo nextest run --all-features --all-targets -- --ignored"
  fi

  echo ""
  echo "🚀 Running nextest tests..."
  if [ "$VERBOSE" = true ]; then
    eval "$NEXTEST_CMD --nocapture"
  else
    eval "$NEXTEST_CMD"
  fi

  # Run doctests if requested
  if [ "$INCLUDE_DOCTESTS" = true ]; then
    echo ""
    echo "📚 Running doctests..."
    if [ "$VERBOSE" = true ]; then
      cargo test --doc --workspace --all-features -- --nocapture
    else
      cargo test --doc --workspace --all-features
    fi
  fi
fi

echo ""
echo "========================================="
echo "✅ All tests completed successfully!"
echo "========================================="
