output "site_url" {
  description = "Frontend URL"
  value       = module.site.url
}

output "api_endpoint" {
  description = "API endpoint"
  value       = "https://${local.api_domain}"
}

output "state_machine_arn" {
  description = "Step Functions state machine ARN"
  value       = aws_sfn_state_machine.pipeline.arn
}

output "cognito_user_pool_id" {
  description = "Cognito user pool ID"
  value       = module.ctx.cognito_user_pool_id
}

output "cognito_client_id" {
  description = "SVAP Cognito app client ID"
  value       = module.cognito.client_id
}
