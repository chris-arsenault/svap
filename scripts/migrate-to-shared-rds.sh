#!/usr/bin/env bash
# One-time migration of svap database from per-project RDS to shared platform RDS.
#
# Usage:
#   source .env  # AWS credentials
#   ./scripts/migrate-to-shared-rds.sh <old-rds-host> <old-rds-password>
#
# Prerequisites:
#   - pg_dump and psql installed
#   - Network access to both old RDS (public) and new RDS (via VPN)

set -euo pipefail

if [ $# -lt 2 ]; then
  echo "Usage: $0 <old-rds-host> <old-rds-password>"
  echo ""
  echo "Get these from the current terraform state before applying consolidation:"
  echo "  terraform output rds_endpoint"
  echo "  terraform output -raw database_url"
  exit 1
fi

OLD_HOST="$1"
OLD_PASS="$2"
OLD_PORT=5432
OLD_USER=svap
OLD_DB=svap
REGION=us-east-1

echo "==> Reading shared RDS details from SSM"
SHARED_HOST=$(aws ssm get-parameter --name /platform/rds/address --query Parameter.Value --output text --region "${REGION}")
SHARED_PORT=$(aws ssm get-parameter --name /platform/rds/port --query Parameter.Value --output text --region "${REGION}")
SHARED_USER=$(aws ssm get-parameter --name /platform/rds/master-username --query Parameter.Value --output text --region "${REGION}")
SHARED_PASS=$(aws ssm get-parameter --name /platform/rds/master-password --with-decryption --query Parameter.Value --output text --region "${REGION}")

DUMP_FILE="/tmp/svap-db-dump.sql"

echo "==> Dumping svap database from old RDS (${OLD_HOST})"
PGPASSWORD="${OLD_PASS}" pg_dump \
  -h "${OLD_HOST}" \
  -p "${OLD_PORT}" \
  -U "${OLD_USER}" \
  -d "${OLD_DB}" \
  --no-owner \
  --no-acl \
  -f "${DUMP_FILE}"

echo "==> Creating svap database on shared RDS (${SHARED_HOST})"
PGPASSWORD="${SHARED_PASS}" psql \
  -h "${SHARED_HOST}" \
  -p "${SHARED_PORT}" \
  -U "${SHARED_USER}" \
  -d postgres \
  -c "CREATE DATABASE svap;" 2>/dev/null || echo "    Database already exists"

echo "==> Restoring dump to shared RDS"
PGPASSWORD="${SHARED_PASS}" psql \
  -h "${SHARED_HOST}" \
  -p "${SHARED_PORT}" \
  -U "${SHARED_USER}" \
  -d svap \
  -f "${DUMP_FILE}"

rm "${DUMP_FILE}"

echo ""
echo "==> Migration complete."
echo "    Verify: PGPASSWORD='${SHARED_PASS}' psql -h ${SHARED_HOST} -U ${SHARED_USER} -d svap"
