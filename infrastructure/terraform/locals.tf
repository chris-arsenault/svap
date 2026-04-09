locals {
  domain_name = "ahara.io"
  prefix      = "svap"

  # Frontend
  hostname = "svap.${local.domain_name}"

  # API
  api_domain = "api.svap.${local.domain_name}"

  # Data
  data_bucket = "${local.prefix}-data"

  # Lambda
  api_lambda_timeout   = 120
  stage_runner_timeout = 900
  stage_runner_memory  = 1024

  # DB URL constructed from platform-context RDS outputs + per-project SSM creds
  db_url = "postgresql://${nonsensitive(data.aws_ssm_parameter.db_username.value)}:${data.aws_ssm_parameter.db_password.value}@${module.ctx.rds_address}:${module.ctx.rds_port}/${nonsensitive(data.aws_ssm_parameter.db_database.value)}?sslmode=require"
}
