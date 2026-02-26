# Cognito Authentication for SVAP

**Date:** 2026-02-25
**Status:** Approved

## Goal

Add Cognito authentication to SVAP so that:
- All API endpoints require a valid JWT
- The UI shows only "SVAP" + login form when unauthenticated (no HHS/OIG info)
- The existing Cognito user pool from `../websites` is reused with a new `svap` app client
- Auth is enforced at the API Gateway level (JWT authorizer)

## Architecture

### Terraform

1. **websites repo** — Add `svap` to `cognito_clients` map in `locals.tf`
2. **svap repo** — Pull Cognito values via `terraform_remote_state` from the websites state (S3 bucket `svap-tfstate-559098897826`, key `ahara-static-websites.tfstate`)
3. **api-http module** — Add optional JWT authorizer support:
   - New variables: `jwt_issuer`, `jwt_audience` (both optional, null = no auth)
   - `aws_apigatewayv2_authorizer` resource (type JWT, Cognito issuer URL)
   - Wire `authorization_type` + `authorizer_id` on all routes
4. **svap.tf** — Pass Cognito pool ID + client ID to:
   - `module "api"` (for JWT authorizer config)
   - `module "site"` runtime_config (for frontend)
   - Lambda environment (for backend awareness)

### Frontend

**New dependency:** `amazon-cognito-identity-js`

**New files:**
- `src/config.ts` — Runtime config reader (same pattern as Scorchbook)
- `src/auth.ts` — Cognito SDK wrapper (signIn, signOut, getSession) — direct port from Scorchbook

**Modified files:**
- `index.html` — Add `<script src="/config.js"></script>` before module script
- `public/config.js` — Empty placeholder for local dev
- `src/App.tsx` — Auth state machine:
  - `loading` → check existing session via `getSession()`
  - `signedOut` → render splash: "SVAP" title + email/password login form. No description, no HHS/OIG text.
  - `signedIn` → render current app (Sidebar + views), pass token down
- `src/data/usePipelineData.tsx` — Accept token, send `Authorization: Bearer {token}` on all API calls

### Backend

Minimal changes — API Gateway JWT authorizer handles validation:
- Add `COGNITO_USER_POOL_ID` and `COGNITO_CLIENT_ID` to Lambda env vars
- CORS already allows all headers (`allow_headers=["*"]`)
- Health endpoint also requires auth (per requirement: no unauthenticated routes)

### Local Development

- Vite proxy to backend continues to work (no API Gateway locally)
- Backend does not enforce auth locally (no changes needed)
- Frontend reads from `VITE_COGNITO_USER_POOL_ID` / `VITE_COGNITO_CLIENT_ID` env vars for local Cognito testing
- `public/config.js` provides empty defaults for dev

## Key Decisions

- **API Gateway JWT authorizer** over backend middleware — enforced at infrastructure level, no Python auth code, impossible to bypass
- **Reuse existing pool** — add `svap` client to shared pool, same users across apps
- **Match Scorchbook pattern** — `amazon-cognito-identity-js` SDK, embedded login form, Bearer tokens
- **No unauthenticated routes** — every endpoint including `/api/health` requires JWT
- **Splash page** — just "SVAP" text and login form, no indication of purpose

## Files Changed

### websites repo
- `infrastructure/terraform/locals.tf` — add `svap` to `cognito_clients`

### svap repo — Terraform
- `infrastructure/terraform/locals.tf` — add cognito locals
- `infrastructure/terraform/svap.tf` — add remote_state data, wire cognito to api module and site module
- `infrastructure/terraform/modules/api-http/variables.tf` — add jwt_issuer, jwt_audience vars
- `infrastructure/terraform/modules/api-http/main.tf` — add authorizer resource, wire to routes

### svap repo — Frontend
- `frontend/package.json` — add amazon-cognito-identity-js
- `frontend/index.html` — add config.js script tag
- `frontend/public/config.js` — dev placeholder
- `frontend/src/config.ts` — new: runtime config reader
- `frontend/src/auth.ts` — new: Cognito wrapper
- `frontend/src/App.tsx` — auth gate + splash screen
- `frontend/src/index.css` — splash/login styles
- `frontend/src/data/usePipelineData.tsx` — add auth token to API calls
- `frontend/src/types.ts` — update Window type declaration (move from usePipelineData)

### svap repo — Backend
- `infrastructure/terraform/svap.tf` — add COGNITO env vars to Lambda
