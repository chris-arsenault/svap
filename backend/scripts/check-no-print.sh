#!/usr/bin/env bash
# Guard: backend-logging-inconsistency (ADR-017)
# All stage files must use logging module, not print()
cd "$(dirname "$0")/.."
if grep -Pn '\bprint\(' src/svap/stages/*.py 2>/dev/null; then
  echo "ERROR: print() found in stage files. Use logger.info/error/warning instead."
  exit 1
fi
echo "OK: No print() calls in stage files."
