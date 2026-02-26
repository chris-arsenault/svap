locals {
  site_name       = element(split(".", var.hostname), 0)
  resource_prefix = trimsuffix(var.bucket_name, "-frontend")
  default_tags = {
    Project   = "StackAtlas"
    ManagedBy = "Terraform"
    Site      = local.site_name
  }

  site_files = {
    for file in fileset(var.site_directory_path, "**") :
    file => file
    if file != "config.js"
  }

  mime_types = {
    ".html"  = "text/html"
    ".css"   = "text/css"
    ".js"    = "application/javascript"
    ".json"  = "application/json"
    ".svg"   = "image/svg+xml"
    ".png"   = "image/png"
    ".jpg"   = "image/jpeg"
    ".jpeg"  = "image/jpeg"
    ".gif"   = "image/gif"
    ".ico"   = "image/x-icon"
    ".xml"   = "application/xml"
    ".txt"   = "text/plain"
    ".pdf"   = "application/pdf"
    ".woff"  = "font/woff"
    ".woff2" = "font/woff2"
    ".ttf"   = "font/ttf"
    ".eot"   = "application/vnd.ms-fontobject"
    ".map"   = "application/json"
  }
}
