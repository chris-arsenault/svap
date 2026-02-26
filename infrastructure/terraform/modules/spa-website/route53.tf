resource "aws_route53_record" "cert_validation" {
  for_each = toset([var.hostname])

  allow_overwrite = true
  name = one([
    for dvo in aws_acm_certificate.website.domain_validation_options : dvo.resource_record_name
    if dvo.domain_name == each.value
  ])
  records = [one([
    for dvo in aws_acm_certificate.website.domain_validation_options : dvo.resource_record_value
    if dvo.domain_name == each.value
  ])]
  ttl = 60
  type = one([
    for dvo in aws_acm_certificate.website.domain_validation_options : dvo.resource_record_type
    if dvo.domain_name == each.value
  ])
  zone_id = data.aws_route53_zone.main.zone_id
}

resource "aws_route53_record" "website" {
  zone_id = data.aws_route53_zone.main.zone_id
  name    = var.hostname
  type    = "A"

  alias {
    name                   = aws_cloudfront_distribution.website.domain_name
    zone_id                = aws_cloudfront_distribution.website.hosted_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "website_ipv6" {
  zone_id = data.aws_route53_zone.main.zone_id
  name    = var.hostname
  type    = "AAAA"

  alias {
    name                   = aws_cloudfront_distribution.website.domain_name
    zone_id                = aws_cloudfront_distribution.website.hosted_zone_id
    evaluate_target_health = false
  }
}
