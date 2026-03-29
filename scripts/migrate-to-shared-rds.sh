#!/usr/bin/env bash
# One-time migration of svap from per-project RDS to shared platform RDS.
#
# Run BEFORE applying the consolidation terraform changes.
# Requires: AWS credentials, pg_dump, platform bin on PATH.
# Does NOT require VPN — restore runs via Lambda in the VPC.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TF_DIR="${REPO_ROOT}/infrastructure/terraform"
MIGRATIONS_DIR="${REPO_ROOT}/db/migrations"
REGION="us-east-1"

# --- Init terraform ---

echo "==> Initializing Terraform"
terraform -chdir="${TF_DIR}" init -reconfigure \
  -backend-config="bucket=tfstate-559098897826" \
  -backend-config="region=${REGION}" \
  -backend-config="use_lockfile=true" \
  > /dev/null

# --- Read old RDS from terraform state ---

echo "==> Reading old RDS details from Terraform state"
OLD_ENDPOINT=$(terraform -chdir="${TF_DIR}" output -raw rds_endpoint)
OLD_HOST="${OLD_ENDPOINT%%:*}"
OLD_PORT="${OLD_ENDPOINT##*:}"
OLD_DB_URL=$(terraform -chdir="${TF_DIR}" output -raw database_url)
OLD_PASS=$(echo "${OLD_DB_URL}" | sed -n 's|.*://[^:]*:\([^@]*\)@.*|\1|p')

echo "    Old host: ${OLD_HOST}"

# --- Dump from old RDS (publicly accessible) ---

DUMP_FILE=$(mktemp --suffix=.sql)
SCHEMA_FILE=$(mktemp --suffix=.sql)

echo "==> Dumping svap database (data + schema, INSERT format)"
PGPASSWORD="${OLD_PASS}" pg_dump \
  -h "${OLD_HOST}" -p "${OLD_PORT}" -U svap -d svap \
  --inserts --rows-per-insert=1000 \
  --no-owner --no-acl --no-comments \
  -f "${DUMP_FILE}"

echo "==> Dumping svap schema only (for baseline migration file)"
PGPASSWORD="${OLD_PASS}" pg_dump \
  -h "${OLD_HOST}" -p "${OLD_PORT}" -U svap -d svap \
  --schema-only --no-owner --no-acl --no-comments \
  -f "${SCHEMA_FILE}"

# --- Create baseline migration file before restore ---

echo "==> Creating baseline migration file"
mkdir -p "${MIGRATIONS_DIR}"
cp "${SCHEMA_FILE}" "${MIGRATIONS_DIR}/001_baseline.sql"
rm "${SCHEMA_FILE}"

# --- Restore via Lambda (handles data + migration baselining atomically) ---

echo "==> Restoring to shared RDS via Lambda"
cd "${REPO_ROOT}"
db-restore "${DUMP_FILE}" "Data migrated from standalone svap RDS during platform consolidation"
rm "${DUMP_FILE}"

echo ""
echo "==> Migration complete."
echo "    - Data restored to shared RDS"
echo "    - Baseline migration created at db/migrations/001_baseline.sql"
echo "    - All migration files baselined in tracking table"
echo ""
echo "Next: apply the consolidated terraform to remove old VPC/RDS"
