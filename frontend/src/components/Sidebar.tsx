import { NavLink } from "react-router-dom";
import {
  LayoutDashboard,
  Database,
  Radar,
  FileSearch,
  FolderTree,
  Tags,
  Layers,
  Grid,
  AlertTriangle,
  Radio,
  Microscope,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { StageDot } from "./SharedUI";
import { usePipelineStatus, useCounts } from "../data/usePipelineSelectors";
import type { Counts } from "../types";

type NavSection = { section: string };
type NavItem = NavSection | {
  path: string;
  label: string;
  icon: LucideIcon;
  countKey?: keyof Counts;
};

function isSection(item: NavSection | NavItem): item is NavSection {
  return "section" in item;
}

const NAV_ITEMS: NavItem[] = [
  { section: "Overview" },
  { path: "/", label: "Dashboard", icon: LayoutDashboard },

  { section: "Corpus" },
  { path: "/sources", label: "Sources", icon: Database },
  { path: "/discovery", label: "Discovery", icon: Radar },
  { path: "/cases", label: "Cases", icon: FileSearch, countKey: "cases" },
  { path: "/policies", label: "Policies", icon: FolderTree, countKey: "policies" },

  { section: "Analysis" },
  { path: "/taxonomy", label: "Taxonomy", icon: Tags, countKey: "taxonomy_qualities" },
  { path: "/dimensions", label: "Dimensions", icon: Layers },
  { path: "/matrix", label: "Convergence", icon: Grid },

  { section: "Results" },
  { path: "/predictions", label: "Predictions", icon: AlertTriangle, countKey: "predictions" },
  { path: "/detection", label: "Detection", icon: Radio, countKey: "detection_patterns" },

  { section: "Research" },
  { path: "/research", label: "Deep Research", icon: Microscope },

  { section: "System" },
  { path: "/management", label: "Management", icon: Settings },
];

const STAGE_NAMES: Record<number, string> = {
  0: "Source Fetching",
  1: "Case Assembly",
  2: "Taxonomy Extraction",
  3: "Convergence Scoring",
  40: "Policy Triage",
  41: "Deep Research",
  42: "Quality Assessment",
  4: "Policy Scanning",
  5: "Exploitation Prediction",
  6: "Detection Patterns",
};

interface SidebarProps {
  onSignOut: () => void;
  username: string;
}

export default function Sidebar({ onSignOut, username }: SidebarProps) {
  const pipeline_status = usePipelineStatus();
  const counts = useCounts();

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <h1>SVAP</h1>
      </div>

      <nav className="sidebar-nav">
        {NAV_ITEMS.map((item, i) => {
          if (isSection(item)) {
            return (
              <div key={i} className="nav-section-label">
                {item.section}
              </div>
            );
          }
          const Icon = item.icon;
          return (
            <NavLink
              key={item.path}
              to={item.path}
              end={item.path === "/"}
              className={({ isActive }) => `nav-item ${isActive ? "active" : ""}`}
            >
              <Icon />
              <span>{item.label}</span>
              {item.countKey && counts?.[item.countKey] > 0 && (
                <span className="nav-badge">{counts[item.countKey]}</span>
              )}
            </NavLink>
          );
        })}
      </nav>

      <div className="pipeline-status">
        <div className="nav-section-label pipeline-status-label">
          Pipeline Status
        </div>
        {(pipeline_status || []).map((s) => (
          <div key={s.stage} className="stage-row">
            <StageDot status={s.status} />
            <span className="stage-row-text">
              {s.stage}. {STAGE_NAMES[s.stage] || `Stage ${s.stage}`}
            </span>
          </div>
        ))}
      </div>
      <div className="sidebar-user">
        <span className="sidebar-user-name">{username}</span>
        <button className="btn" onClick={onSignOut}>Sign out</button>
      </div>
    </aside>
  );
}
