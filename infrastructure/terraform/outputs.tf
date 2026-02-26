output "site_url" {
  description = "Frontend URL"
  value       = module.site.website_url
}

output "api_endpoint" {
  description = "API endpoint"
  value       = "https://${local.api_domain}"
}

output "rds_endpoint" {
  description = "RDS PostgreSQL endpoint"
  value       = aws_db_instance.main.endpoint
}

output "state_machine_arn" {
  description = "Step Functions state machine ARN"
  value       = aws_sfn_state_machine.pipeline.arn
}

output "cognito_user_pool_id" {
  description = "Cognito user pool ID (from websites)"
  value       = local.cognito_user_pool_id
}

output "cognito_client_id" {
  description = "SVAP Cognito app client ID"
  value       = local.cognito_client_id
}
