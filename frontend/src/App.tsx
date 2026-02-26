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
