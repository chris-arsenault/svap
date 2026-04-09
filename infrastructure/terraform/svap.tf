# =============================================================================
# Platform context + Cognito client
# =============================================================================

module "ctx" {
  source = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/platform-context"
}

module "cognito" {
  source  = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/cognito-app"
  name    = "${local.prefix}-app"
  cognito = module.ctx.cognito
}

# =============================================================================
# Per-project DB credentials (not in platform-context — per-project SSM)
# =============================================================================

data "aws_ssm_parameter" "db_username" {
  name = "/ahara/db/svap/username"
}

data "aws_ssm_parameter" "db_password" {
  name = "/ahara/db/svap/password"
}

data "aws_ssm_parameter" "db_database" {
  name = "/ahara/db/svap/database"
}

# =============================================================================
# Data S3 bucket (svap-specific — enforcement source documents)
# =============================================================================

resource "aws_s3_bucket" "data" {
  bucket = local.data_bucket
}

resource "aws_kms_key" "data_bucket" {
  description             = "KMS key for ${local.prefix}-data S3 bucket encryption"
  deletion_window_in_days = 10
  enable_key_rotation     = true
}

resource "aws_kms_alias" "data_bucket" {
  name          = "alias/${local.prefix}-data-bucket"
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
# IAM policy for Lambda functions (shared between API + stage-runner)
# =============================================================================

data "aws_iam_policy_document" "lambda" {
  statement {
    sid = "DataBucketAccess"
    actions = [
      "s3:GetObject",
      "s3:PutObject",
      "s3:ListBucket",
    ]
    resources = [
      aws_s3_bucket.data.arn,
      "${aws_s3_bucket.data.arn}/*",
    ]
  }

  statement {
    sid = "DataBucketKms"
    actions = [
      "kms:Decrypt",
      "kms:GenerateDataKey",
    ]
    resources = [aws_kms_key.data_bucket.arn]
  }

  statement {
    sid = "BedrockInvoke"
    actions = [
      "bedrock:InvokeModel",
    ]
    resources = [
      "arn:aws:bedrock:*::foundation-model/anthropic.*",
      "arn:aws:bedrock:*:*:inference-profile/*",
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
# API Lambda (via ahara-tf-patterns alb-api — shared ALB, jwt-validation)
#
# Replaces the previous API Gateway V2 HTTP API with the shared ALB pattern.
# JWT validation is handled by the ALB's jwt-validation action (same as
# dosekit/tastebase). CORS is handled by the platform-wide CORS Lambda in
# ahara-infra/services.
# =============================================================================

module "api" {
  source   = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/alb-api"
  prefix   = local.prefix
  hostname = local.api_domain

  vpc     = module.ctx.vpc
  alb     = module.ctx.alb
  cognito = module.ctx.cognito

  environment = {
    DATABASE_URL               = local.db_url
    SVAP_CONFIG_BUCKET         = aws_s3_bucket.data.bucket
    PIPELINE_STATE_MACHINE_ARN = aws_sfn_state_machine.pipeline.arn
    COGNITO_USER_POOL_ID       = module.ctx.cognito_user_pool_id
    COGNITO_CLIENT_ID          = module.cognito.client_id
  }

  iam_policy = [data.aws_iam_policy_document.lambda.json]

  lambdas = {
    api = {
      binary = "${path.module}/../../backend/target/lambda/api/bootstrap"
      routes = [
        { priority = 300, paths = ["/api/health"], methods = ["GET"], authenticated = false },
        { priority = 301, paths = ["/api/*"], methods = ["GET", "HEAD"], authenticated = false },
        { priority = 302, paths = ["/api/*"], authenticated = true },
      ]
    }
  }
}

# =============================================================================
# Stage Runner Lambda (standalone — long-running pipeline stages, SFN-invoked)
# =============================================================================

module "stage_runner" {
  source = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/lambda"

  name        = "${local.prefix}-stage-runner"
  binary      = "${path.module}/../../backend/target/lambda/stage-runner/bootstrap"
  role_arn    = module.api.role_arn
  timeout     = local.stage_runner_timeout
  memory_size = local.stage_runner_memory

  vpc = module.ctx.vpc

  environment = {
    DATABASE_URL       = local.db_url
    SVAP_CONFIG_BUCKET = aws_s3_bucket.data.bucket
  }
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
  name               = "${local.prefix}-sfn"
  assume_role_policy = data.aws_iam_policy_document.sfn_assume.json
}

resource "aws_iam_role_policy" "sfn_invoke_lambda" {
  name = "${local.prefix}-sfn-invoke"
  role = aws_iam_role.sfn.id
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["lambda:InvokeFunction"]
      Resource = [module.stage_runner.function_arn]
    }]
  })
}

resource "aws_sfn_state_machine" "pipeline" {
  name     = "${local.prefix}-pipeline"
  role_arn = aws_iam_role.sfn.arn

  definition = jsonencode({
    Comment = "SVAP 7-stage pipeline (0-6) with human gates at stages 2 and 5"
    StartAt = "Stage0_SourceFetch"
    States = {
      Stage0_SourceFetch = {
        Type     = "Task"
        Resource = "arn:aws:states:::lambda:invoke"
        Parameters = {
          FunctionName = module.stage_runner.function_arn
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
          FunctionName = module.stage_runner.function_arn
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
          FunctionName = module.stage_runner.function_arn
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
          FunctionName = module.stage_runner.function_arn
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
          FunctionName = module.stage_runner.function_arn
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
          FunctionName = module.stage_runner.function_arn
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
          FunctionName = module.stage_runner.function_arn
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
          FunctionName = module.stage_runner.function_arn
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
          FunctionName = module.stage_runner.function_arn
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

  tags = { Name = "${local.prefix}-pipeline" }
}

# =============================================================================
# Frontend SPA (via ahara-tf-patterns website module)
# =============================================================================

module "site" {
  source         = "git::https://github.com/chris-arsenault/ahara-tf-patterns.git//modules/website"
  prefix         = local.prefix
  hostname       = local.hostname
  site_directory = "${path.module}/../../frontend/dist"

  runtime_config = {
    apiBaseUrl        = "https://${local.api_domain}/api"
    cognitoUserPoolId = module.ctx.cognito_user_pool_id
    cognitoClientId   = module.cognito.client_id
  }
}
