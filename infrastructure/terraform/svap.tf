# =============================================================================
# Remote state — pull Cognito config from websites project
# =============================================================================

data "terraform_remote_state" "websites" {
  backend = "s3"
  config = {
    bucket = "tf-state-websites-559098897826"
    key    = "ahara-static-websites.tfstate"
    region = "us-east-1"
  }
}

# =============================================================================
# VPC (minimal)
# =============================================================================

data "aws_availability_zones" "available" {
  state = "available"
}

resource "aws_vpc" "main" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_support   = true
  enable_dns_hostnames = true

  tags = { Name = "${local.name_prefix}-vpc" }
}

# --- DB subnets (RDS requires two AZs) ---

resource "aws_subnet" "db_a" {
  vpc_id                  = aws_vpc.main.id
  cidr_block              = "10.0.100.0/24"
  availability_zone       = data.aws_availability_zones.available.names[0]
  map_public_ip_on_launch = true

  tags = { Name = "${local.name_prefix}-db-a" }
}

resource "aws_subnet" "db_b" {
  vpc_id                  = aws_vpc.main.id
  cidr_block              = "10.0.101.0/24"
  availability_zone       = data.aws_availability_zones.available.names[1]
  map_public_ip_on_launch = true

  tags = { Name = "${local.name_prefix}-db-b" }
}

# --- Internet gateway ---

resource "aws_internet_gateway" "main" {
  vpc_id = aws_vpc.main.id

  tags = { Name = "${local.name_prefix}-igw" }
}

# --- Route tables ---

resource "aws_route_table" "public" {
  vpc_id = aws_vpc.main.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.main.id
  }

  tags = { Name = "${local.name_prefix}-public-rt" }
}

resource "aws_route_table_association" "db_a" {
  subnet_id      = aws_subnet.db_a.id
  route_table_id = aws_route_table.public.id
}

resource "aws_route_table_association" "db_b" {
  subnet_id      = aws_subnet.db_b.id
  route_table_id = aws_route_table.public.id
}

# =============================================================================
# RDS PostgreSQL
# =============================================================================

resource "random_password" "db" {
  length  = 32
  special = false
}

resource "aws_security_group" "rds" {
  name_prefix = "${local.name_prefix}-rds-"
  description = "Security group for RDS PostgreSQL (prototype)"
  vpc_id      = aws_vpc.main.id

  ingress {
    from_port   = 5432
    to_port     = 5432
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Allow PostgreSQL from anywhere (prototype - restrict for production)"
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
    description = "Allow all outbound"
  }

  tags = { Name = "${local.name_prefix}-rds-sg" }
}

resource "aws_db_subnet_group" "main" {
  name       = "${local.name_prefix}-db"
  subnet_ids = [aws_subnet.db_a.id, aws_subnet.db_b.id]
  tags       = { Name = "${local.name_prefix}-db-subnet-group" }
}

resource "aws_db_instance" "main" {
  identifier     = "${local.name_prefix}-db"
  engine         = "postgres"
  engine_version = "16"
  instance_class = "db.t4g.micro"

  allocated_storage = 20
  storage_type      = "gp3"

  db_name  = "svap"
  username = "svap"
  password = random_password.db.result

  db_subnet_group_name   = aws_db_subnet_group.main.name
  vpc_security_group_ids = [aws_security_group.rds.id]
  publicly_accessible    = true
  skip_final_snapshot    = true

  tags = { Name = "${local.name_prefix}-db" }
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
      "states:SendTaskSuccess",
      "states:SendTaskFailure",
    ]
    resources = [aws_sfn_state_machine.pipeline.arn]
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
    DATABASE_URL               = "postgresql://${aws_db_instance.main.username}:${random_password.db.result}@${aws_db_instance.main.endpoint}/${aws_db_instance.main.db_name}?sslmode=require"
    SVAP_CONFIG_BUCKET         = aws_s3_bucket.data.bucket
    PIPELINE_STATE_MACHINE_ARN = aws_sfn_state_machine.pipeline.arn
    COGNITO_USER_POOL_ID       = local.cognito_user_pool_id
    COGNITO_CLIENT_ID          = local.cognito_client_id
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

  environment {
    variables = {
      DATABASE_URL       = "postgresql://${aws_db_instance.main.username}:${random_password.db.result}@${aws_db_instance.main.endpoint}/${aws_db_instance.main.db_name}?sslmode=require"
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
        Type     = "Task"
        Resource = "arn:aws:states:::lambda:invoke"
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
        Type     = "Task"
        Resource = "arn:aws:states:::lambda:invoke"
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
