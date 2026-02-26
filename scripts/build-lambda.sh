#!/usr/bin/env bash
# Package Python backend as Lambda deployment zip
set -e

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="$ROOT_DIR/backend/dist"
BUILD_DIR="$(mktemp -d)"

echo "Building Lambda deployment package..."

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# Install dependencies into build dir
# Note: boto3 is already in the Lambda runtime — don't bundle it
uv pip install \
    --target "$BUILD_DIR" \
    --quiet \
    --python-platform x86_64-unknown-linux-gnu \
    --python-version 3.12 \
    --no-build \
    pyyaml tiktoken numpy \
    psycopg2-binary

# Copy source code
cp -r "$ROOT_DIR/backend/src/svap" "$BUILD_DIR/"

# Create zip
cd "$BUILD_DIR"
zip -r -q "$DIST_DIR/lambda-api.zip" . -x "__pycache__/*" "*.pyc" "*.dist-info/*"

echo "Built: $DIST_DIR/lambda-api.zip ($(du -h "$DIST_DIR/lambda-api.zip" | cut -f1))"

# Cleanup
rm -rf "$BUILD_DIR"
