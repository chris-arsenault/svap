variable "hostname" {
  description = "The full hostname for the website (e.g., app.example.com)"
  type        = string
}

variable "domain_name" {
  description = "The root domain name (e.g., example.com)"
  type        = string
}

variable "site_directory_path" {
  description = "Path to the directory containing all website files"
  type        = string
}

variable "runtime_config" {
  description = "Runtime config emitted as config.js"
  type        = map(any)
  default     = {}
}

variable "bucket_name" {
  description = "S3 bucket name for the SPA assets"
  type        = string
}

variable "waf_acl_arn" {
  description = "ARN of a WAF Web ACL to associate with the CloudFront distribution. Leave empty to skip WAF association."
  type        = string
  default     = ""
}
