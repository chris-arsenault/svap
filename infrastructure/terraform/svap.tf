# =============================================================================
# Platform SSM lookups
# =============================================================================

data "aws_ssm_parameter" "cognito_user_pool_id" {
  name = "/platform/cognito/user-pool-id"
}

data "aws_ssm_parameter" "cognito_client_svap" {
  name = "/platform/cognito/clients/svap"
}

data "aws_ssm_parameter" "rds_address" {
  name = "/platform/rds/address"
}

data "aws_ssm_parameter" "rds_port" {
  name = "/platform/rds/port"
}

data "aws_ssm_parameter" "rds_master_username" {
  name = "/platform/rds/master-username"
}

data "aws_ssm_parameter" "rds_master_password" {
  name = "/platform/rds/master-password"
}

data "aws_ssm_parameter" "private_subnet_ids" {
  name = "/platform/network/private-subnet-ids"
}

data "aws_ssm_parameter" "vpc_id" {
  name = "/platform/network/vpc-id"
}

# =============================================================================
# VPC networking (uses shared platform VPC)
# =============================================================================

resource "aws_security_group" "lambda" {
  name        = "${local.name_prefix}-lambda"
  description = "SVAP Lambda functions"
  vpc_id      = nonsensitive(data.aws_ssm_parameter.vpc_id.value)

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

locals {
  db_url = "postgresql://${nonsensitive(data.aws_ssm_parameter.rds_master_username.value)}:${data.aws_ssm_parameter.rds_master_password.value}@${nonsensitive(data.aws_ssm_parameter.rds_address.value)}:${nonsensitive(data.aws_ssm_parameter.rds_port.value)}/svap?sslmode=require"
}

# =============================================================================
# Data S3 bucket
# =============================================================================

resource "aws_s3_bucket" "data" {
  bucket = local.data_bucket
}

resource "aws_kms_key" "data_bucket" {
  description             = "KMS key for ${local.name_prefix}-data S3 bucket encryption"
  deletion_window_in_days = 10
  enable_key_rotation     = true
}

resource "aws_kms_alias" "data_bucket" {
  name          = "alias/${local.name_prefix}-data-bucket"
  target_key_id = aws_kms_key.data_bucket.key_id
}

resource "aws_s3_bucket_server_side_encryption_configuration" "data" {
  bucket = aws_s3_bucket.data.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm     = "aws:kms"
      kms_master_key_id = aws_kms_key.data_bucket.arn
    }
  }
}

resource "aws_s3_bucket_public_access_block" "data" {
  bucket = aws_s3_bucket.data.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

# =============================================================================
# IAM
# =============================================================================

data "aws_iam_policy_document" "lambda" {
  statement {
    sid = "DataBucketAccess"
    actions = [
      "s3:GetObject",
      "s3:PutObject",
      "s3:ListBucket"
    ]
    resources = [
      aws_s3_bucket.data.arn,
      "${aws_s3_bucket.data.arn}/*"
    ]
  }

  statement {
    sid = "DataBucketKms"
    actions = [
      "kms:Decrypt",
      "kms:GenerateDataKey"
    ]
    resources = [aws_kms_key.data_bucket.arn]
  }

  statement {
    sid = "BedrockInvoke"
    actions = [
      "bedrock:InvokeModel"
    ]
    resources = [
      "arn:aws:bedrock:*::foundation-model/anthropic.*",
      "arn:aws:bedrock:*:*:inference-profile/*"
    ]
  }

  statement {
    sid = "StepFunctions"
    actions = [
      "states:StartExecution",
      "states:DescribeExecution",
      "states:ListExecutions",
      "states:StopExecution",
      "states:SendTaskSuccess",
      "states:SendTaskFailure",
    ]
    resources = [
      aws_sfn_state_machine.pipeline.arn,
      replace(aws_sfn_state_machine.pipeline.arn, ":stateMachine:", ":execution:"),
      "${replace(aws_sfn_state_machine.pipeline.arn, ":stateMachine:", ":execution:")}:*",
    ]
  }
}

# =============================================================================
# API Lambda (via api-http module)
# =============================================================================

module "api" {
  source = "./modules/api-http"

  name            = local.name_prefix
  lambda_zip_path = "${path.module}/../../backend/dist/lambda-api.zip"
  lambda_runtime  = local.lambda_runtime
  lambda_handler  = "svap.api.handler"
  lambda_timeout  = local.api_lambda_timeout
  lambda_memory   = local.lambda_memory

  lambda_environment = {
    DATABASE_URL               = local.db_url
    SVAP_CONFIG_BUCKET         = aws_s3_bucket.data.bucket
    PIPELINE_STATE_MACHINE_ARN = aws_sfn_state_machine.pipeline.arn
    COGNITO_USER_POOL_ID       = local.cognito_user_pool_id
    COGNITO_CLIENT_ID          = local.cognito_client_id
  }

  vpc_config = {
    subnet_ids         = split(",", nonsensitive(data.aws_ssm_parameter.private_subnet_ids.value))
    security_group_ids = [aws_security_group.lambda.id]
  }

  iam_policy_json = data.aws_iam_policy_document.lambda.json

  routes = [
    "GET /api/dashboard",
    "GET /api/status",
    "GET /api/cases",
    "GET /api/cases/{case_id}",
    "GET /api/taxonomy",
    "GET /api/taxonomy/{quality_id}",
    "GET /api/convergence/cases",
    "GET /api/convergence/policies",
    "GET /api/policies",
    "GET /api/policies/{policy_id}",
    "GET /api/predictions",
    "GET /api/detection-patterns",
    "GET /api/hhs/policy-catalog",
    "GET /api/hhs/policy-catalog/flat",
    "GET /api/hhs/enforcement-sources",
    "GET /api/hhs/data-sources",
    "GET /api/enforcement-sources",
    "POST /api/enforcement-sources",
    "POST /api/enforcement-sources/upload",
    "POST /api/enforcement-sources/delete",
    "POST /api/pipeline/run",
    "POST /api/pipeline/approve",
    "POST /api/pipeline/seed",
    "GET /api/health",
    "POST /api/discovery/run-feeds",
    "GET /api/discovery/candidates",
    "POST /api/discovery/candidates/review",
    "GET /api/discovery/feeds",
    "POST /api/discovery/feeds",
    "POST /api/research/triage",
    "POST /api/research/deep",
    "GET /api/research/triage",
    "GET /api/research/sessions",
    "GET /api/research/findings/{policy_id}",
    "GET /api/research/assessments/{policy_id}",
    "GET /api/dimensions",
    "GET /api/management/executions",
    "POST /api/management/executions/stop",
    "GET /api/management/runs",
    "POST /api/management/runs/delete",
  ]

  cors_allow_origins = local.allowed_origins
  jwt_issuer   = local.cognito_issuer
  jwt_audience = [local.cognito_client_id]
  custom_domain_name = local.api_domain
  domain_zone_name   = local.domain_name
}

# =============================================================================
# Stage Runner Lambda (standalone — long-running pipeline stages)
# =============================================================================

resource "aws_lambda_function" "stage_runner" {
  function_name    = "${local.name_prefix}-stage-runner"
  role             = module.api.lambda_role_arn
  handler          = "svap.stage_runner.handler"
  runtime          = local.lambda_runtime
  filename         = "${path.module}/../../backend/dist/lambda-api.zip"
  source_code_hash = filebase64sha256("${path.module}/../../backend/dist/lambda-api.zip")
  timeout          = local.stage_runner_timeout
  memory_size      = 1024

  vpc_config {
    subnet_ids         = split(",", nonsensitive(data.aws_ssm_parameter.private_subnet_ids.value))
    security_group_ids = [aws_security_group.lambda.id]
  }

  environment {
    variables = {
      DATABASE_URL       = local.db_url
      SVAP_CONFIG_BUCKET = aws_s3_bucket.data.bucket
    }
  }

  tags = { Name = "${local.name_prefix}-stage-runner" }
}

# =============================================================================
# Step Functions — Pipeline Orchestrator
# =============================================================================

data "aws_iam_policy_document" "sfn_assume" {
  statement {
    effect = "Allow"
    principals {
      type        = "Service"
      identifiers = ["states.amazonaws.com"]
    }
    actions = ["sts:AssumeRole"]
  }
}

resource "aws_iam_role" "sfn" {
  name               = "${local.name_prefix}-sfn"
  assume_role_policy = data.aws_iam_policy_document.sfn_assume.json
}

resource "aws_iam_role_policy" "sfn_invoke_lambda" {
  name = "${local.name_prefix}-sfn-invoke"
  role = aws_iam_role.sfn.id
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["lambda:InvokeFunction"]
      Resource = [aws_lambda_function.stage_runner.arn]
    }]
  })
}

resource "aws_sfn_state_machine" "pipeline" {
  name     = "${local.name_prefix}-pipeline"
  role_arn = aws_iam_role.sfn.arn

  definition = jsonencode({
    Comment = "SVAP 7-stage pipeline (0-6) with human gates at stages 2 and 5"
    StartAt = "Stage0_SourceFetch"
    States = {
      Stage0_SourceFetch = {
        Type     = "Task"
        Resource = "arn:aws:states:::lambda:invoke"
        Parameters = {
          FunctionName = aws_lambda_function.stage_runner.arn
          Payload = {
            "run_id.$" = "$.run_id"
            stage      = 0
          }
        }
        ResultPath = "$.stage0_result"
        Next       = "Stage1_CaseAssembly"
      }
      Stage1_CaseAssembly = {
        Type     = "Task"
        Resource = "arn:aws:states:::lambda:invoke"
        Parameters = {
          FunctionName = aws_lambda_function.stage_runner.arn
          Payload = {
            "run_id.$" = "$.run_id"
            stage      = 1
          }
        }
        ResultPath = "$.stage1_result"
        Next       = "Stage2_TaxonomyExtraction"
      }
      Stage2_TaxonomyExtraction = {
        Type     = "Task"
        Resource = "arn:aws:states:::lambda:invoke"
        Parameters = {
          FunctionName = aws_lambda_function.stage_runner.arn
          Payload = {
            "run_id.$" = "$.run_id"
            stage      = 2
          }
        }
        ResultPath = "$.stage2_result"
        Next       = "Gate2_HumanReview"
      }
      Gate2_HumanReview = {
        Type     = "Task"
        Resource = "arn:aws:states:::lambda:invoke.waitForTaskToken"
        Parameters = {
          FunctionName = aws_lambda_function.stage_runner.arn
          Payload = {
            "run_id.$"     = "$.run_id"
            stage          = 2
            gate           = true
            "task_token.$" = "$$.Task.Token"
          }
        }
        ResultPath = "$.gate2_result"
        Next       = "Stage3_ConvergenceScoring"
      }
      Stage3_ConvergenceScoring = {
        Type     = "Task"
        Resource = "arn:aws:states:::lambda:invoke"
        Parameters = {
          FunctionName = aws_lambda_function.stage_runner.arn
          Payload = {
            "run_id.$" = "$.run_id"
            stage      = 3
          }
        }
        ResultPath = "$.stage3_result"
        Next       = "Stage4_PolicyScanning"
      }
      Stage4_PolicyScanning = {
        Type     = "Task"
        Resource = "arn:aws:states:::lambda:invoke"
        Parameters = {
          FunctionName = aws_lambda_function.stage_runner.arn
          Payload = {
            "run_id.$" = "$.run_id"
            stage      = 4
          }
        }
        ResultPath = "$.stage4_result"
        Next       = "Stage5_ExploitationPrediction"
      }
      Stage5_ExploitationPrediction = {
        Type           = "Task"
        Resource       = "arn:aws:states:::lambda:invoke"
        TimeoutSeconds = 960
        Parameters = {
          FunctionName = aws_lambda_function.stage_runner.arn
          Payload = {
            "run_id.$" = "$.run_id"
            stage      = 5
          }
        }
        ResultPath = "$.stage5_result"
        Next       = "Gate5_HumanReview"
      }
      Gate5_HumanReview = {
        Type     = "Task"
        Resource = "arn:aws:states:::lambda:invoke.waitForTaskToken"
        Parameters = {
          FunctionName = aws_lambda_function.stage_runner.arn
          Payload = {
            "run_id.$"     = "$.run_id"
            stage          = 5
            gate           = true
            "task_token.$" = "$$.Task.Token"
          }
        }
        ResultPath = "$.gate5_result"
        Next       = "Stage6_DetectionPatterns"
      }
      Stage6_DetectionPatterns = {
        Type           = "Task"
        Resource       = "arn:aws:states:::lambda:invoke"
        TimeoutSeconds = 960
        Parameters = {
          FunctionName = aws_lambda_function.stage_runner.arn
          Payload = {
            "run_id.$" = "$.run_id"
            stage      = 6
          }
        }
        ResultPath = "$.stage6_result"
        End        = true
      }
    }
  })

  tags = { Name = "${local.name_prefix}-pipeline" }
}

# =============================================================================
# Frontend SPA (via spa-website module)
# =============================================================================

module "site" {
  source = "./modules/spa-website"

  hostname            = local.hostname
  domain_name         = local.domain_name
  site_directory_path = "${path.module}/../../frontend/dist"
  bucket_name         = local.frontend_bucket

  runtime_config = {
    apiBaseUrl        = "https://${local.api_domain}/api"
    cognitoUserPoolId = local.cognito_user_pool_id
    cognitoClientId   = local.cognito_client_id
  }
}
