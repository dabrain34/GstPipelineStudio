#!/bin/bash
# Test runner for GstPipelineStudio
#
# Due to GTK initialization constraints, the test modules need to run separately.
# Each test module initializes GTK in its own ThreadPool, and GTK can only be
# initialized once per process.

set -e

echo "Running GPS tests (element + player tests)..."
cargo test gps::test -- --test-threads=1 "$@"

echo ""
echo "Running GraphManager tests..."
cargo test graphmanager::test -- --test-threads=1 "$@"

echo ""
echo "✓ All tests passed!"
