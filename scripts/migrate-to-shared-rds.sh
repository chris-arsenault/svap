#!/usr/bin/env bash
# One-time migration of svap database from per-project RDS to shared platform RDS.
#
# Run this BEFORE applying the consolidation terraform changes.
#
# Prerequisites:
#   - AWS credentials (source .env)
#   - pg_dump and psql installed
#   - Terraform initialized (old state still active)
#   - Network access to both old RDS (public) and new RDS (via VPN)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TF_DIR="${SCRIPT_DIR}/../infrastructure/terraform"
REGION=us-east-1

echo "==> Reading old RDS details from Terraform state"
OLD_ENDPOINT=$(terraform -chdir="${TF_DIR}" output -raw rds_endpoint)
OLD_HOST="${OLD_ENDPOINT%%:*}"
OLD_PORT="${OLD_ENDPOINT##*:}"
OLD_DB_URL=$(terraform -chdir="${TF_DIR}" output -raw database_url)
OLD_PASS=$(echo "${OLD_DB_URL}" | sed -n 's|.*://[^:]*:\([^@]*\)@.*|\1|p')
OLD_USER=svap
OLD_DB=svap

echo "    Host: ${OLD_HOST}"
echo "    Port: ${OLD_PORT}"

echo "==> Reading shared RDS details from SSM"
SHARED_HOST=$(aws ssm get-parameter --name /platform/rds/address --query Parameter.Value --output text --region "${REGION}")
SHARED_PORT=$(aws ssm get-parameter --name /platform/rds/port --query Parameter.Value --output text --region "${REGION}")
SHARED_USER=$(aws ssm get-parameter --name /platform/rds/master-username --query Parameter.Value --output text --region "${REGION}")
SHARED_PASS=$(aws ssm get-parameter --name /platform/rds/master-password --with-decryption --query Parameter.Value --output text --region "${REGION}")

echo "    Host: ${SHARED_HOST}"

DUMP_FILE="/tmp/svap-db-dump.sql"

echo "==> Dumping svap database from old RDS"
PGPASSWORD="${OLD_PASS}" pg_dump \
  -h "${OLD_HOST}" \
  -p "${OLD_PORT}" \
  -U "${OLD_USER}" \
  -d "${OLD_DB}" \
  --no-owner \
  --no-acl \
  -f "${DUMP_FILE}"

echo "==> Creating svap database on shared RDS"
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
echo "==> Migration complete. Now apply the consolidation terraform."
