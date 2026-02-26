# Cognito Authentication Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Cognito authentication to SVAP so all API endpoints require a valid JWT and the UI gates everything behind a minimal "SVAP" splash + login form.

**Architecture:** Reuse the existing Cognito user pool from `../websites` by adding a new `svap` app client. Enforce auth at the API Gateway level via a JWT authorizer (no Python auth code needed). Frontend uses `amazon-cognito-identity-js` (same as Scorchbook) with an embedded login form.

**Tech Stack:** Terraform (API Gateway JWT authorizer), React + `amazon-cognito-identity-js`, FastAPI (unchanged except env vars)

---

### Task 1: Add `svap` Cognito client to websites repo

**Files:**
- Modify: `../websites/infrastructure/terraform/locals.tf:26-28` (cognito_clients map)

**Step 1: Add svap to the clients map**

In `/home/tsonu/src/websites/infrastructure/terraform/locals.tf`, change the `cognito_clients` map from:

```hcl
  cognito_clients = {
    scorchbook = "${local.scorchbook_name_prefix}-app"
  }
```

to:

```hcl
  cognito_clients = {
    scorchbook = "${local.scorchbook_name_prefix}-app"
    svap       = "svap-app"
  }
```

**Step 2: Apply in websites terraform**

Run: `cd /home/tsonu/src/websites/infrastructure/terraform && terraform plan -target='module.cognito'`
Expected: Plan shows 1 new resource: `aws_cognito_user_pool_client.clients["svap"]`

Run: `terraform apply -target='module.cognito'`

**Step 3: Commit**

```bash
cd /home/tsonu/src/websites
git add infrastructure/terraform/locals.tf
git commit -m "feat: add svap Cognito app client to shared pool"
```

---

### Task 2: Add JWT authorizer support to api-http module

**Files:**
- Modify: `infrastructure/terraform/modules/api-http/variables.tf`
- Modify: `infrastructure/terraform/modules/api-http/main.tf`

**Step 1: Add authorizer variables**

Append to `infrastructure/terraform/modules/api-http/variables.tf`:

```hcl
variable "jwt_issuer" {
  description = "JWT issuer URL for API Gateway authorizer (e.g. Cognito pool URL). Null disables auth."
  type        = string
  default     = null
}

variable "jwt_audience" {
  description = "JWT audience (e.g. Cognito app client ID). Required if jwt_issuer is set."
  type        = list(string)
  default     = []
}
```

**Step 2: Add authorizer resource and wire to routes**

In `infrastructure/terraform/modules/api-http/main.tf`, add after the `aws_apigatewayv2_integration` resource (after line 105):

```hcl
resource "aws_apigatewayv2_authorizer" "jwt" {
  count            = var.jwt_issuer != null ? 1 : 0
  api_id           = aws_apigatewayv2_api.api.id
  authorizer_type  = "JWT"
  identity_sources = ["$request.header.Authorization"]
  name             = "${local.safe_name}-jwt"

  jwt_configuration {
    issuer   = var.jwt_issuer
    audience = var.jwt_audience
  }
}
```

Then modify the `aws_apigatewayv2_route` resource (lines 107-112) from:

```hcl
resource "aws_apigatewayv2_route" "routes" {
  for_each  = toset(var.routes)
  api_id    = aws_apigatewayv2_api.api.id
  route_key = each.value
  target    = "integrations/${aws_apigatewayv2_integration.api.id}"
}
```

to:

```hcl
resource "aws_apigatewayv2_route" "routes" {
  for_each           = toset(var.routes)
  api_id             = aws_apigatewayv2_api.api.id
  route_key          = each.value
  target             = "integrations/${aws_apigatewayv2_integration.api.id}"
  authorization_type = var.jwt_issuer != null ? "JWT" : "NONE"
  authorizer_id      = var.jwt_issuer != null ? aws_apigatewayv2_authorizer.jwt[0].id : null
}
```

**Step 3: Commit**

```bash
cd /home/tsonu/src/svap
git add infrastructure/terraform/modules/api-http/
git commit -m "feat: add optional JWT authorizer support to api-http module"
```

---

### Task 3: Wire Cognito into SVAP Terraform

**Files:**
- Modify: `infrastructure/terraform/svap.tf` (add remote state, pass cognito to modules)
- Modify: `infrastructure/terraform/locals.tf` (add cognito locals)
- Modify: `infrastructure/terraform/outputs.tf` (add cognito outputs)

**Step 1: Add remote state data source**

At the top of `infrastructure/terraform/svap.tf` (before the VPC section, insert at line 1):

```hcl
# =============================================================================
# Remote state — pull Cognito config from websites project
# =============================================================================

data "terraform_remote_state" "websites" {
  backend = "s3"
  config = {
    bucket = "svap-tfstate-559098897826"
    key    = "ahara-static-websites.tfstate"
    region = "us-east-1"
  }
}

```

**Step 2: Add cognito locals**

In `infrastructure/terraform/locals.tf`, add after the existing locals (before the closing `}`):

```hcl
  # Cognito (from websites remote state)
  cognito_user_pool_id = data.terraform_remote_state.websites.outputs.cognito_user_pool_id
  cognito_client_id    = data.terraform_remote_state.websites.outputs.cognito_client_ids["svap"]
  cognito_issuer       = "https://cognito-idp.us-east-1.amazonaws.com/${local.cognito_user_pool_id}"
```

Note: This requires the websites project to expose these outputs. Check if they exist; if not, add them (see step 2a).

**Step 2a: Ensure websites outputs exist**

Check `/home/tsonu/src/websites/infrastructure/terraform/outputs.tf` for `cognito_user_pool_id` and `cognito_client_ids`. If they don't exist, add:

```hcl
output "cognito_user_pool_id" {
  description = "Cognito user pool ID"
  value       = module.cognito.user_pool_id
}

output "cognito_client_ids" {
  description = "Map of Cognito client keys to app client IDs"
  value       = module.cognito.client_ids
}
```

**Step 3: Pass JWT config to api module**

In `infrastructure/terraform/svap.tf`, modify the `module "api"` block to add JWT authorizer config and Cognito env vars. Add these lines inside the module block:

After the `cors_allow_origins` line (around line 260), add:

```hcl
  jwt_issuer   = local.cognito_issuer
  jwt_audience = [local.cognito_client_id]
```

Inside the `lambda_environment` map, add:

```hcl
    COGNITO_USER_POOL_ID       = local.cognito_user_pool_id
    COGNITO_CLIENT_ID          = local.cognito_client_id
```

**Step 4: Pass Cognito config to frontend site module**

In `infrastructure/terraform/svap.tf`, modify the `module "site"` block's `runtime_config`:

Change from:

```hcl
  runtime_config = {
    apiBaseUrl = "https://${local.api_domain}"
  }
```

to:

```hcl
  runtime_config = {
    apiBaseUrl         = "https://${local.api_domain}"
    cognitoUserPoolId  = local.cognito_user_pool_id
    cognitoClientId    = local.cognito_client_id
  }
```

**Step 5: Add cognito outputs**

Append to `infrastructure/terraform/outputs.tf`:

```hcl
output "cognito_user_pool_id" {
  description = "Cognito user pool ID (from websites)"
  value       = local.cognito_user_pool_id
}

output "cognito_client_id" {
  description = "SVAP Cognito app client ID"
  value       = local.cognito_client_id
}
```

**Step 6: Validate**

Run: `cd /home/tsonu/src/svap/infrastructure/terraform && terraform validate`
Expected: Success

Run: `terraform plan`
Expected: Changes to api module (new authorizer, route updates), site module (config.js update), Lambda env var updates.

**Step 7: Commit**

```bash
cd /home/tsonu/src/svap
git add infrastructure/terraform/
git commit -m "feat: wire Cognito JWT authorizer into SVAP API Gateway and frontend config"
```

---

### Task 4: Add frontend config and auth modules

**Files:**
- Create: `frontend/public/config.js`
- Create: `frontend/src/config.ts`
- Create: `frontend/src/auth.ts`
- Modify: `frontend/index.html`
- Modify: `frontend/package.json`

**Step 1: Install amazon-cognito-identity-js**

Run: `cd /home/tsonu/src/svap/frontend && npm install amazon-cognito-identity-js`

**Step 2: Create public/config.js placeholder**

Create `frontend/public/config.js`:

```javascript
// Runtime config — overwritten by Terraform in production (S3 deployment).
// For local dev, values come from VITE_* env vars in config.ts.
window.__APP_CONFIG__ = window.__APP_CONFIG__ || {};
```

**Step 3: Add config.js script to index.html**

In `frontend/index.html`, add the config.js script tag before the module script. Change:

```html
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
```

to:

```html
    <div id="root"></div>
    <script src="/config.js"></script>
    <script type="module" src="/src/main.tsx"></script>
```

Also change the title from `SVAP — HHS OIG Workstation` to just `SVAP`:

```html
    <title>SVAP</title>
```

**Step 4: Create config.ts**

Create `frontend/src/config.ts`:

```typescript
type RuntimeConfig = {
  apiBaseUrl?: string;
  cognitoUserPoolId?: string;
  cognitoClientId?: string;
};

declare global {
  interface Window {
    __APP_CONFIG__?: RuntimeConfig;
  }
}

const runtimeConfig = typeof window !== "undefined" ? window.__APP_CONFIG__ : undefined;
const readRuntime = (value?: string) => (value && value.trim().length > 0 ? value : undefined);

export const config = {
  apiBaseUrl: readRuntime(runtimeConfig?.apiBaseUrl) ?? import.meta.env.VITE_API_BASE_URL ?? "",
  cognitoUserPoolId: readRuntime(runtimeConfig?.cognitoUserPoolId) ?? import.meta.env.VITE_COGNITO_USER_POOL_ID ?? "",
  cognitoClientId: readRuntime(runtimeConfig?.cognitoClientId) ?? import.meta.env.VITE_COGNITO_CLIENT_ID ?? "",
};
```

**Step 5: Create auth.ts**

Create `frontend/src/auth.ts`:

```typescript
import {
  CognitoUserPool,
  CognitoUser,
  AuthenticationDetails,
  CognitoUserSession,
} from "amazon-cognito-identity-js";
import { config } from "./config";

const getUserPool = () => {
  if (!config.cognitoUserPoolId || !config.cognitoClientId) {
    throw new Error("Missing Cognito configuration");
  }
  return new CognitoUserPool({
    UserPoolId: config.cognitoUserPoolId,
    ClientId: config.cognitoClientId,
  });
};

const getCurrentUser = () => {
  try {
    return getUserPool().getCurrentUser();
  } catch {
    return null;
  }
};

export const signIn = (username: string, password: string): Promise<CognitoUserSession> => {
  const authenticationDetails = new AuthenticationDetails({
    Username: username,
    Password: password,
  });

  const user = new CognitoUser({
    Username: username,
    Pool: getUserPool(),
  });

  return new Promise((resolve, reject) => {
    user.authenticateUser(authenticationDetails, {
      onSuccess: (session) => resolve(session),
      onFailure: (error) => reject(error),
    });
  });
};

export const signOut = () => {
  const user = getCurrentUser();
  user?.signOut();
};

export const getSession = (): Promise<CognitoUserSession | null> => {
  const user = getCurrentUser();
  if (!user) {
    return Promise.resolve(null);
  }
  return new Promise((resolve) => {
    user.getSession((error: Error | null, session: CognitoUserSession | null) => {
      if (error || !session) {
        resolve(null);
        return;
      }
      resolve(session);
    });
  });
};
```

**Step 6: Commit**

```bash
cd /home/tsonu/src/svap
git add frontend/public/config.js frontend/src/config.ts frontend/src/auth.ts frontend/index.html frontend/package.json frontend/package-lock.json
git commit -m "feat: add Cognito auth module and runtime config for frontend"
```

---

### Task 5: Add auth gate and splash screen to App.tsx

**Files:**
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/index.css` (add splash/login styles)

**Step 1: Rewrite App.tsx with auth gate**

Replace `frontend/src/App.tsx` entirely:

```tsx
import { useState, useEffect, type FormEvent } from "react";
import { PipelineProvider } from "./data/usePipelineData";
import Sidebar from "./components/Sidebar";
import Dashboard from "./views/Dashboard";
import CaseSourcing from "./views/CaseSourcing";
import PolicyExplorer from "./views/PolicyExplorer";
import TaxonomyView from "./views/TaxonomyView";
import ConvergenceMatrix from "./views/ConvergenceMatrix";
import PredictionView from "./views/PredictionView";
import DetectionView from "./views/DetectionView";
import { signIn, signOut, getSession } from "./auth";
import type { ViewId, ViewProps } from "./types";

type AuthState = {
  status: "loading" | "signedOut" | "signedIn";
  token: string;
  username: string;
};

const VIEWS: Record<ViewId, React.ComponentType<ViewProps>> = {
  dashboard: Dashboard,
  cases: CaseSourcing,
  policies: PolicyExplorer,
  taxonomy: TaxonomyView,
  matrix: ConvergenceMatrix,
  predictions: PredictionView,
  detection: DetectionView,
};

export default function App() {
  const [auth, setAuth] = useState<AuthState>({ status: "loading", token: "", username: "" });
  const [activeView, setActiveView] = useState<ViewId>("dashboard");
  const [errorMessage, setErrorMessage] = useState("");

  useEffect(() => {
    const loadSession = async () => {
      try {
        const session = await getSession();
        if (session) {
          const payload = session.getIdToken().payload as Record<string, unknown>;
          const displayName =
            (typeof payload.name === "string" && payload.name) ||
            (typeof payload.email === "string" && payload.email) ||
            (typeof payload["cognito:username"] === "string" && payload["cognito:username"]) ||
            "";
          setAuth({ status: "signedIn", token: session.getIdToken().getJwtToken(), username: displayName });
          return;
        }
      } catch (error) {
        console.error("Session load failed:", error);
      }
      setAuth({ status: "signedOut", token: "", username: "" });
    };
    loadSession();
  }, []);

  const handleSignIn = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const formData = new FormData(event.currentTarget);
    const username = String(formData.get("username") ?? "");
    const password = String(formData.get("password") ?? "");
    setErrorMessage("");
    try {
      const session = await signIn(username, password);
      const payload = session.getIdToken().payload as Record<string, unknown>;
      const displayName =
        (typeof payload.name === "string" && payload.name) ||
        (typeof payload.email === "string" && payload.email) ||
        "";
      setAuth({ status: "signedIn", token: session.getIdToken().getJwtToken(), username: displayName || username });
    } catch (error) {
      setErrorMessage((error as Error).message || "Sign in failed");
    }
  };

  const handleSignOut = () => {
    signOut();
    setAuth({ status: "signedOut", token: "", username: "" });
  };

  // Loading state
  if (auth.status === "loading") {
    return (
      <div className="splash-screen">
        <div className="splash-title">SVAP</div>
      </div>
    );
  }

  // Unauthenticated — splash + login
  if (auth.status === "signedOut") {
    return (
      <div className="splash-screen">
        <div className="splash-card">
          <div className="splash-title">SVAP</div>
          <form className="login-form" onSubmit={handleSignIn}>
            <input name="username" type="email" placeholder="Email" required autoComplete="username" />
            <input name="password" type="password" placeholder="Password" required autoComplete="current-password" />
            {errorMessage && <div className="login-error">{errorMessage}</div>}
            <button type="submit" className="login-btn">Sign in</button>
          </form>
        </div>
      </div>
    );
  }

  // Authenticated — full app
  const ViewComponent = VIEWS[activeView] || Dashboard;
  return (
    <PipelineProvider token={auth.token}>
      <div className="app-layout">
        <Sidebar activeView={activeView} onNavigate={setActiveView} onSignOut={handleSignOut} username={auth.username} />
        <main className="main-content" key={activeView}>
          <ViewComponent onNavigate={setActiveView} />
        </main>
      </div>
    </PipelineProvider>
  );
}
```

**Step 2: Add splash/login CSS**

Append to `frontend/src/index.css` (after the last line):

```css
/* Splash / Login screen */
.splash-screen {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  background: var(--bg-root);
}

.splash-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--sp-8);
}

.splash-title {
  font-family: var(--font-display);
  font-size: 32px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--accent-bright);
}

.login-form {
  display: flex;
  flex-direction: column;
  gap: var(--sp-3);
  width: 280px;
}

.login-form input {
  font-family: var(--font-body);
  font-size: 13px;
  padding: var(--sp-3) var(--sp-4);
  background: var(--bg-card);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-primary);
  outline: none;
  transition: border-color 0.15s;
}

.login-form input:focus {
  border-color: var(--accent);
}

.login-form input::placeholder {
  color: var(--text-muted);
}

.login-btn {
  font-family: var(--font-display);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  padding: var(--sp-3) var(--sp-4);
  background: var(--accent-dim);
  border: 1px solid var(--accent);
  border-radius: var(--radius-md);
  color: var(--accent-bright);
  cursor: pointer;
  transition: all 0.15s;
}

.login-btn:hover {
  background: var(--accent);
  color: var(--text-inverse);
}

.login-error {
  font-size: 12px;
  color: var(--critical);
  text-align: center;
}
```

**Step 3: Commit**

```bash
cd /home/tsonu/src/svap
git add frontend/src/App.tsx frontend/src/index.css
git commit -m "feat: add auth gate with splash screen and login form"
```

---

### Task 6: Update Sidebar with sign-out button

**Files:**
- Modify: `frontend/src/components/Sidebar.tsx`

**Step 1: Update Sidebar props and add sign-out**

The Sidebar needs two new props: `onSignOut` and `username`. Update `frontend/src/components/Sidebar.tsx`:

Change the interface from:

```typescript
interface SidebarProps {
  activeView: ViewId;
  onNavigate: (view: ViewId) => void;
}
```

to:

```typescript
interface SidebarProps {
  activeView: ViewId;
  onNavigate: (view: ViewId) => void;
  onSignOut: () => void;
  username: string;
}
```

Change the function signature from:

```typescript
export default function Sidebar({ activeView, onNavigate }: SidebarProps) {
```

to:

```typescript
export default function Sidebar({ activeView, onNavigate, onSignOut, username }: SidebarProps) {
```

Change the sidebar-header section from:

```tsx
      <div className="sidebar-header">
        <h1>SVAP</h1>
        <div className="subtitle">HHS OIG Workstation</div>
      </div>
```

to:

```tsx
      <div className="sidebar-header">
        <h1>SVAP</h1>
      </div>
```

Add a user section at the bottom, after the closing `</div>` of `pipeline-status` and before `</aside>`:

```tsx
      <div className="sidebar-user">
        <span className="sidebar-user-name">{username}</span>
        <button className="btn" onClick={onSignOut}>Sign out</button>
      </div>
```

**Step 2: Add sidebar-user CSS**

Append to `frontend/src/index.css` (after the login styles from Task 5):

```css
/* Sidebar user section */
.sidebar-user {
  padding: var(--sp-3) var(--sp-4);
  border-top: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--sp-2);
}

.sidebar-user-name {
  font-size: 11px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
```

**Step 3: Commit**

```bash
cd /home/tsonu/src/svap
git add frontend/src/components/Sidebar.tsx frontend/src/index.css
git commit -m "feat: add sign-out button and remove HHS subtitle from sidebar"
```

---

### Task 7: Add auth token to API calls in usePipelineData

**Files:**
- Modify: `frontend/src/data/usePipelineData.tsx`

**Step 1: Update PipelineProvider to accept token and pass it through**

In `frontend/src/data/usePipelineData.tsx`:

Remove the `Window` type declaration (lines 23-27) since it's now in `config.ts`:

```typescript
declare global {
  interface Window {
    __APP_CONFIG__?: { apiBaseUrl?: string };
  }
}
```

Replace the `API_BASE` line with an import:

```typescript
import { config } from "../config";

const API_BASE = config.apiBaseUrl || "/api";
```

Change `usePipelineData` to accept a token parameter. Change:

```typescript
export function usePipelineData(): PipelineData {
```

to:

```typescript
export function usePipelineData(token: string): PipelineData {
```

Update `fetchDashboard` to include the Authorization header. Change:

```typescript
      const res = await fetch(`${API_BASE}/dashboard`, { signal: AbortSignal.timeout(3000) });
```

to:

```typescript
      const res = await fetch(`${API_BASE}/dashboard`, {
        signal: AbortSignal.timeout(3000),
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
```

Update `apiPost` to accept and send the token. Change:

```typescript
async function apiPost(path: string, body?: unknown): Promise<unknown> {
  const options: RequestInit = { method: "POST" };
  if (body !== undefined) {
    options.headers = { "Content-Type": "application/json" };
    options.body = JSON.stringify(body);
  }
  const res = await fetch(`${API_BASE}${path}`, options);
```

to:

```typescript
async function apiPost(path: string, body?: unknown, token?: string): Promise<unknown> {
  const headers: Record<string, string> = {};
  if (body !== undefined) headers["Content-Type"] = "application/json";
  if (token) headers["Authorization"] = `Bearer ${token}`;
  const options: RequestInit = { method: "POST", headers };
  if (body !== undefined) options.body = JSON.stringify(body);
  const res = await fetch(`${API_BASE}${path}`, options);
```

Update `runStage`, `approveStage`, and `seedPipeline` to pass the token. Change each call from `apiPost("/pipeline/...", ...)` to `apiPost("/pipeline/...", ..., token)`:

For `runStage`:
```typescript
  const runStage = useCallback(
    async (stage: number) => {
      if (!apiAvailable) throw new Error("API not available");
      return apiPost("/pipeline/run", { stage }, token);
    },
    [apiAvailable, token]
  );
```

For `approveStage`:
```typescript
  const approveStage = useCallback(
    async (stage: number) => {
      if (!apiAvailable) throw new Error("API not available");
      const result = await apiPost("/pipeline/approve", { stage }, token);
      await fetchDashboard();
      return result;
    },
    [apiAvailable, fetchDashboard, token]
  );
```

For `seedPipeline`:
```typescript
  const seedPipeline = useCallback(async () => {
    if (!apiAvailable) throw new Error("API not available");
    const result = await apiPost("/pipeline/seed", undefined, token);
    await fetchDashboard();
    return result;
  }, [apiAvailable, fetchDashboard, token]);
```

Update `PipelineProvider` to accept and pass the token. Change:

```typescript
export function PipelineProvider({ children }: { children: React.ReactNode }) {
  const pipeline = usePipelineData();
```

to:

```typescript
export function PipelineProvider({ children, token }: { children: React.ReactNode; token: string }) {
  const pipeline = usePipelineData(token);
```

**Step 2: Commit**

```bash
cd /home/tsonu/src/svap
git add frontend/src/data/usePipelineData.tsx
git commit -m "feat: add Bearer token to all API requests"
```

---

### Task 8: Verify frontend builds and typecheck

**Step 1: Run typecheck**

Run: `cd /home/tsonu/src/svap/frontend && npx tsc --noEmit`
Expected: No errors. If there are type errors, fix them.

**Step 2: Run build**

Run: `cd /home/tsonu/src/svap/frontend && npm run build`
Expected: Build succeeds, outputs to `dist/`

**Step 3: Run lint**

Run: `cd /home/tsonu/src/svap/frontend && npm run lint`
Expected: No errors. Fix any lint issues.

**Step 4: Commit any fixes**

If any fixes were needed:
```bash
git add frontend/
git commit -m "fix: resolve type/lint issues from auth integration"
```

---

### Task 9: Check websites outputs and apply Terraform

**Step 1: Verify websites outputs**

Check if `/home/tsonu/src/websites/infrastructure/terraform/outputs.tf` has `cognito_user_pool_id` and `cognito_client_ids` outputs. If not, add them:

```hcl
output "cognito_user_pool_id" {
  description = "Cognito user pool ID"
  value       = module.cognito.user_pool_id
}

output "cognito_client_ids" {
  description = "Map of Cognito client keys to app client IDs"
  value       = module.cognito.client_ids
}
```

**Step 2: Apply websites Terraform (if outputs were added)**

Run: `cd /home/tsonu/src/websites/infrastructure/terraform && terraform apply`

**Step 3: Validate SVAP Terraform**

Run: `cd /home/tsonu/src/svap/infrastructure/terraform && terraform validate`
Expected: Success

**Step 4: Plan SVAP Terraform**

Run: `cd /home/tsonu/src/svap/infrastructure/terraform && terraform plan`
Expected: Shows new authorizer, updated routes with JWT auth, updated config.js, updated Lambda env vars.

**Step 5: Commit websites changes if any**

```bash
cd /home/tsonu/src/websites
git add infrastructure/terraform/outputs.tf
git commit -m "feat: expose Cognito outputs for cross-project consumption"
```

---

### Task 10: Deploy

**Step 1: Build frontend**

Run: `cd /home/tsonu/src/svap/frontend && npm run build`

**Step 2: Build backend Lambda zip**

Run: `cd /home/tsonu/src/svap && make build` (or whatever the build process is — check Makefile/scripts)

**Step 3: Apply Terraform**

Run: `cd /home/tsonu/src/svap/infrastructure/terraform && terraform apply`

Review the plan carefully — should show:
- New `aws_apigatewayv2_authorizer.jwt[0]`
- Updated routes with `authorization_type = JWT`
- Updated Lambda env vars (COGNITO_USER_POOL_ID, COGNITO_CLIENT_ID)
- Updated S3 config.js (new cognitoUserPoolId, cognitoClientId values)
- Updated frontend files in S3

**Step 4: Verify**

1. Visit `https://svap.ahara.io` — should see "SVAP" splash + login form, nothing else
2. Try accessing `https://api.svap.ahara.io/api/health` directly — should get 401 Unauthorized
3. Sign in with valid credentials — should see full dashboard
4. Verify API calls work when signed in (check browser network tab for 200s)

**Step 5: Final commit**

```bash
cd /home/tsonu/src/svap
git add -A
git commit -m "feat: complete Cognito auth integration for SVAP"
```
