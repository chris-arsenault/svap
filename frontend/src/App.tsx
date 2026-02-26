import { useState, useEffect, useCallback, type FormEvent } from "react";
import { PipelineProvider, usePipeline } from "./data/usePipelineData";
import Sidebar from "./components/Sidebar";
import Dashboard from "./views/Dashboard";
import SourcesView from "./views/SourcesView";
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
  sources: SourcesView,
  cases: CaseSourcing,
  policies: PolicyExplorer,
  taxonomy: TaxonomyView,
  matrix: ConvergenceMatrix,
  predictions: PredictionView,
  detection: DetectionView,
};

export default function App() {
  const [auth, setAuth] = useState<AuthState>({ status: "loading", token: "", username: "" });
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

  const handleSignOut = useCallback(() => {
    signOut();
    setAuth({ status: "signedOut", token: "", username: "" });
  }, []);

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
  return <AuthenticatedApp token={auth.token} username={auth.username} onSignOut={handleSignOut} />;
}

function AuthenticatedApp({ token, username, onSignOut }: { token: string; username: string; onSignOut: () => void }) {
  const [activeView, setActiveView] = useState<ViewId>("dashboard");
  const [sidebarOpen, setSidebarOpen] = useState(false);

  const handleNavigate = useCallback((view: ViewId) => {
    setActiveView(view);
    setSidebarOpen(false);
  }, []);

  const ViewComponent = VIEWS[activeView] || Dashboard;
  return (
    <PipelineProvider token={token}>
      <div className={`app-layout ${sidebarOpen ? "sidebar-open" : ""}`}>
        {/* eslint-disable-next-line jsx-a11y/no-static-element-interactions, jsx-a11y/click-events-have-key-events */}
        <div className="sidebar-overlay" onClick={() => setSidebarOpen(false)} />
        <Sidebar
          activeView={activeView}
          onNavigate={handleNavigate}
          onSignOut={onSignOut}
          username={username}
        />
        <main className="main-content" key={activeView}>
          <button
            className="mobile-menu-btn"
            onClick={() => setSidebarOpen((o) => !o)}
            aria-label="Toggle menu"
          >
            <span className="hamburger-icon" />
          </button>
          <ApiGate>
            <ViewComponent onNavigate={setActiveView} />
          </ApiGate>
        </main>
      </div>
    </PipelineProvider>
  );
}

function ApiGate({ children }: { children: React.ReactNode }) {
  const { loading, error, refresh } = usePipeline();

  if (loading) {
    return (
      <div className="api-gate">
        <div className="api-gate-spinner" />
        <p>Connecting to SVAP API&hellip;</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="api-gate">
        <h2>Service Unavailable</h2>
        <p>{error}</p>
        <button className="btn btn-accent" onClick={refresh}>Retry</button>
      </div>
    );
  }

  return <>{children}</>;
}
