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

  # Cognito (from websites remote state)
  cognito_user_pool_id = data.terraform_remote_state.websites.outputs.cognito_user_pool_id
  cognito_client_id    = data.terraform_remote_state.websites.outputs.cognito_client_ids["svap"]
  cognito_issuer       = "https://cognito-idp.us-east-1.amazonaws.com/${local.cognito_user_pool_id}"
}
