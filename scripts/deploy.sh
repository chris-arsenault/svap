#!/usr/bin/env bash
# deploy.sh - Build backend + frontend, sync migrations, deploy via Terraform
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

STATE_BUCKET="${STATE_BUCKET:-tfstate-559098897826}"
STATE_REGION="${STATE_REGION:-us-east-1}"

# ── Build Lambda zip ─────────────────────────────────────────────────
echo "==> Building Lambda deployment package"
bash "$SCRIPT_DIR/build-lambda.sh"

# ── Build frontend ───────────────────────────────────────────────────
echo ""
echo "==> Building frontend"
cd "$REPO_ROOT/frontend"

if [ -d "dist" ]; then
  rm -rf dist
fi

npm install --silent
npm run build

if [ ! -d "dist" ]; then
  echo "    ERROR: Missing dist directory after build"
  exit 1
fi

index_hits=$(grep -n -E "@vite/client|/@react-refresh" dist/index.html 2>/dev/null || true)
if [ -n "$index_hits" ]; then
  echo "    ERROR: Dev server markers found in index.html"
  echo "$index_hits"
  exit 1
fi

js_hits=$(grep -R -n -I -E "react-refresh|jsx-dev-runtime|@vite/client" dist --include='*.js' 2>/dev/null || true)
if [ -n "$js_hits" ]; then
  echo "    ERROR: Dev-only runtime markers found in JS bundle"
  echo "$js_hits" | head -n 20
  exit 1
fi

echo "    Frontend build OK"

# ── Sync database migrations ─────────────────────────────────────────
if [ -d "$REPO_ROOT/db/migrations" ]; then
  echo ""
  echo "==> Uploading database migrations"
  MIGRATIONS_BUCKET=$(aws ssm get-parameter --name /platform/db/migrations-bucket \
    --query Parameter.Value --output text --region "${STATE_REGION}")
  aws s3 sync "$REPO_ROOT/db/migrations/" \
    "s3://${MIGRATIONS_BUCKET}/migrations/svap/" \
    --delete
fi

# ── Deploy with Terraform ────────────────────────────────────────────
echo ""
echo "==> Running Terraform"
cd "$REPO_ROOT/infrastructure/terraform"
terraform init -reconfigure \
  -backend-config="bucket=${STATE_BUCKET}" \
  -backend-config="region=${STATE_REGION}" \
  -backend-config="use_lockfile=true"
terraform apply -auto-approve

echo ""
echo "==> Deployment complete!"
