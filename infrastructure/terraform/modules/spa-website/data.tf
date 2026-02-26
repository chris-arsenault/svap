data "aws_route53_zone" "main" {
  name         = "${var.domain_name}."
  private_zone = false
}

data "aws_caller_identity" "current" {}
