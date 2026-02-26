#!/usr/bin/env bash
# ensure-state-bucket.sh - Create/verify the S3 state bucket for Terraform remote state.
# Follows the standardized pattern from the-canonry/scripts/ensure-state-bucket.sh.
#
# Usage:
#   ./scripts/ensure-state-bucket.sh
#
# Environment overrides:
#   STATE_BUCKET  - bucket name (default: svap-tfstate-<account-id>)
#   STATE_REGION  - bucket region (default: us-east-1)

set -euo pipefail

AWS_ACCOUNT_ID=$(aws sts get-caller-identity --query Account --output text)
STATE_BUCKET="${STATE_BUCKET:-svap-tfstate-${AWS_ACCOUNT_ID}}"
STATE_REGION="${STATE_REGION:-us-east-1}"

echo "State bucket: ${STATE_BUCKET}"
echo "State region: ${STATE_REGION}"

if aws s3api head-bucket --bucket "${STATE_BUCKET}" 2>/dev/null; then
  echo "Bucket already exists."
else
  echo "Creating state bucket: ${STATE_BUCKET}"
  aws s3api create-bucket \
    --bucket "${STATE_BUCKET}" \
    --region "${STATE_REGION}" \
    $([ "${STATE_REGION}" != "us-east-1" ] && echo "--create-bucket-configuration LocationConstraint=${STATE_REGION}")

  aws s3api put-bucket-versioning \
    --bucket "${STATE_BUCKET}" \
    --versioning-configuration Status=Enabled

  aws s3api put-public-access-block \
    --bucket "${STATE_BUCKET}" \
    --public-access-block-configuration \
      BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=true,RestrictPublicBuckets=true

  echo "Bucket created with versioning and public access block."
fi

echo ""
echo "Use with terraform init:"
echo "  terraform init -backend-config=\"bucket=${STATE_BUCKET}\" -backend-config=\"region=${STATE_REGION}\""
