# WAF Web ACL attached to CloudFront. Currently default-allow with no custom rules.
# Exists to satisfy Trivy's security requirement that CloudFront distributions
# have WAF protection. Add rate-limiting or IP-blocking rules here as needed.
resource "aws_wafv2_web_acl" "cloudfront" {
  name        = "${local.resource_prefix}-cf-waf"
  description = "WAF Web ACL for ${var.hostname} CloudFront distribution"
  scope       = "CLOUDFRONT"

  default_action {
    allow {}
  }

  visibility_config {
    cloudwatch_metrics_enabled = true
    metric_name                = "${replace(local.resource_prefix, "-", "")}CfWaf"
    sampled_requests_enabled   = true
  }

  tags = merge(local.default_tags, {
    Name = "${local.resource_prefix}-cf-waf"
  })
}

resource "aws_cloudfront_origin_access_control" "website" {
  name                              = "${local.resource_prefix}-oac"
  description                       = "OAC for ${var.hostname}"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

resource "aws_cloudfront_distribution" "website" {
  enabled             = true
  is_ipv6_enabled     = true
  default_root_object = "index.html"
  aliases             = [var.hostname]
  price_class         = "PriceClass_100"
  web_acl_id          = var.waf_acl_arn != "" ? var.waf_acl_arn : aws_wafv2_web_acl.cloudfront.arn

  origin {
    domain_name              = aws_s3_bucket.website.bucket_regional_domain_name
    origin_id                = "S3-${local.resource_prefix}"
    origin_access_control_id = aws_cloudfront_origin_access_control.website.id
  }

  default_cache_behavior {
    allowed_methods        = ["GET", "HEAD", "OPTIONS"]
    cached_methods         = ["GET", "HEAD"]
    target_origin_id       = "S3-${local.resource_prefix}"
    viewer_protocol_policy = "redirect-to-https"
    compress               = true

    forwarded_values {
      query_string = false
      cookies {
        forward = "none"
      }
    }

    min_ttl     = 0
    default_ttl = 3600
    max_ttl     = 86400
  }

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  viewer_certificate {
    acm_certificate_arn      = aws_acm_certificate.website.arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }

  # SPA routing: CloudFront returns index.html for 404/403 errors so that
  # React Router can handle client-side routing (e.g. /projects/p1/edit).
  # Without this, refreshing on a deep link returns a CloudFront error page.
  custom_error_response {
    error_code         = 404
    response_code      = 200
    response_page_path = "/index.html"
  }

  custom_error_response {
    error_code         = 403
    response_code      = 200
    response_page_path = "/index.html"
  }

  tags = merge(local.default_tags, {
    Name = "${local.resource_prefix}-cloudfront"
  })
}
