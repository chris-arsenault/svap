locals {
  domain_name = "ahara.io"
  name_prefix = "svap"

  # Frontend
  hostname        = "svap.${local.domain_name}"
  frontend_bucket = "${local.name_prefix}-frontend"

  # API
  api_domain = "api.svap.${local.domain_name}"

  # Data
  data_bucket = "${local.name_prefix}-data"

  # CORS
  allowed_origins = [
    "http://localhost:5173",
    "https://${local.hostname}"
  ]

  # Lambda
  lambda_runtime       = "python3.12"
  api_lambda_timeout   = 120
  stage_runner_timeout = 900
  lambda_memory        = 512

  # Cognito (from platform SSM)
  cognito_user_pool_id = nonsensitive(data.aws_ssm_parameter.cognito_user_pool_id.value)
  cognito_client_id    = nonsensitive(data.aws_ssm_parameter.cognito_client_svap.value)
  cognito_issuer       = "https://cognito-idp.us-east-1.amazonaws.com/${local.cognito_user_pool_id}"
}
