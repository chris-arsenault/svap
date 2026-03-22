#!/usr/bin/env bash
# One-time migration of svap from per-project RDS to shared platform RDS.
#
# Run BEFORE applying the consolidation terraform changes.
# Requires: AWS credentials, pg_dump, psql, VPN connected, platform bin on PATH.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TF_DIR="${REPO_ROOT}/infrastructure/terraform"
MIGRATIONS_DIR="${REPO_ROOT}/db/migrations"
REGION="us-east-1"

# --- Init terraform against old state ---

echo "==> Initializing Terraform"
terraform -chdir="${TF_DIR}" init -reconfigure \
  -backend-config="bucket=tfstate-559098897826" \
  -backend-config="region=${REGION}" \
  -backend-config="use_lockfile=true" \
  > /dev/null

# --- Read old RDS from current terraform state ---

echo "==> Reading old RDS details from Terraform state"
OLD_ENDPOINT=$(terraform -chdir="${TF_DIR}" output -raw rds_endpoint)
OLD_HOST="${OLD_ENDPOINT%%:*}"
OLD_PORT="${OLD_ENDPOINT##*:}"
OLD_DB_URL=$(terraform -chdir="${TF_DIR}" output -raw database_url)
OLD_PASS=$(echo "${OLD_DB_URL}" | sed -n 's|.*://[^:]*:\([^@]*\)@.*|\1|p')

echo "    Old host: ${OLD_HOST}"

# --- Read shared RDS from SSM ---

echo "==> Reading shared RDS details from SSM"
SHARED_HOST=$(aws ssm get-parameter --name /platform/rds/address --query Parameter.Value --output text --region "${REGION}")
SHARED_PORT=$(aws ssm get-parameter --name /platform/rds/port --query Parameter.Value --output text --region "${REGION}")
SHARED_USER=$(aws ssm get-parameter --name /platform/rds/master-username --query Parameter.Value --output text --region "${REGION}")
SHARED_PASS=$(aws ssm get-parameter --name /platform/rds/master-password --with-decryption --query Parameter.Value --output text --region "${REGION}")

echo "    Shared host: ${SHARED_HOST}"

# --- Dump data + schema from old RDS ---

DUMP_FILE=$(mktemp)
SCHEMA_FILE=$(mktemp)

echo "==> Dumping svap database (data + schema)"
PGPASSWORD="${OLD_PASS}" pg_dump \
  -h "${OLD_HOST}" -p "${OLD_PORT}" -U svap -d svap \
  --no-owner --no-acl \
  -f "${DUMP_FILE}"

echo "==> Dumping svap schema only (for baseline migration)"
PGPASSWORD="${OLD_PASS}" pg_dump \
  -h "${OLD_HOST}" -p "${OLD_PORT}" -U svap -d svap \
  --schema-only --no-owner --no-acl \
  -f "${SCHEMA_FILE}"

# --- Restore to shared RDS ---

echo "==> Testing shared RDS connectivity"
PGPASSWORD="${SHARED_PASS}" psql \
  -h "${SHARED_HOST}" -p "${SHARED_PORT}" -U "${SHARED_USER}" -d postgres \
  -c "SELECT 1;" > /dev/null || { echo "ERROR: Cannot connect to shared RDS. Is the VPN connected?"; exit 1; }

echo "==> Creating svap database on shared RDS"
PGPASSWORD="${SHARED_PASS}" psql \
  -h "${SHARED_HOST}" -p "${SHARED_PORT}" -U "${SHARED_USER}" -d postgres \
  -c "CREATE DATABASE svap;" 2>&1 | grep -v "already exists" || true

echo "==> Restoring dump to shared RDS"
PGPASSWORD="${SHARED_PASS}" psql \
  -h "${SHARED_HOST}" -p "${SHARED_PORT}" -U "${SHARED_USER}" -d svap \
  -f "${DUMP_FILE}"

rm "${DUMP_FILE}"

# --- Create baseline migration file ---

echo "==> Creating baseline migration"
mkdir -p "${MIGRATIONS_DIR}"
cp "${SCHEMA_FILE}" "${MIGRATIONS_DIR}/001_baseline.sql"
rm "${SCHEMA_FILE}"

# --- Upload and record as noop ---

echo "==> Recording baseline as noop"
cd "${REPO_ROOT}"
db-noop 001_baseline.sql "Schema restored from standalone svap RDS during platform consolidation"

echo ""
echo "==> Migration complete."
echo "    - Data restored to shared RDS"
echo "    - Baseline migration created at db/migrations/001_baseline.sql"
echo "    - Baseline recorded in migration tracking (noop)"
echo ""
echo "Next: apply the consolidated terraform to remove old VPC/RDS"
