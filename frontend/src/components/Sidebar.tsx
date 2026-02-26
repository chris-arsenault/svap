import React from "react";
import {
  LayoutDashboard,
  Database,
  FileSearch,
  FolderTree,
  Tags,
  Grid,
  AlertTriangle,
  Radio,
  type LucideIcon,
} from "lucide-react";
import { StageDot } from "./SharedUI";
import { usePipelineStatus, useCounts } from "../data/usePipelineSelectors";
import type { Counts, ViewId } from "../types";

type NavSection = { section: string };
type NavLink = {
  id: ViewId;
  label: string;
  icon: LucideIcon;
  countKey?: keyof Counts;
};
type NavItem = NavSection | NavLink;

function isSection(item: NavItem): item is NavSection {
  return "section" in item;
}

const NAV_ITEMS: NavItem[] = [
  { section: "Analysis" },
  { id: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { id: "sources", label: "Sources", icon: Database },
  { id: "cases", label: "Case Sourcing", icon: FileSearch, countKey: "cases" },
  { id: "policies", label: "Policy Explorer", icon: FolderTree },
  { section: "Pipeline Results" },
  { id: "taxonomy", label: "Taxonomy", icon: Tags, countKey: "taxonomy_qualities" },
  { id: "matrix", label: "Convergence Matrix", icon: Grid },
  { id: "predictions", label: "Predictions", icon: AlertTriangle, countKey: "predictions" },
  { id: "detection", label: "Detection Patterns", icon: Radio, countKey: "detection_patterns" },
];

const STAGE_NAMES: Record<number, string> = {
  0: "Source Fetching",
  1: "Case Assembly",
  2: "Taxonomy Extraction",
  3: "Convergence Scoring",
  4: "Policy Scanning",
  5: "Exploitation Prediction",
  6: "Detection Patterns",
};

interface SidebarProps {
  activeView: ViewId;
  onNavigate: (view: ViewId) => void;
  onSignOut: () => void;
  username: string;
}

export default function Sidebar({ activeView, onNavigate, onSignOut, username }: SidebarProps) {
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
            <div
              key={item.id}
              className={`nav-item ${activeView === item.id ? "active" : ""}`}
              role="button"
              tabIndex={0}
              onClick={() => onNavigate(item.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onNavigate(item.id);
                }
              }}
            >
              <Icon />
              <span>{item.label}</span>
              {item.countKey && counts?.[item.countKey] > 0 && (
                <span className="nav-badge">{counts[item.countKey]}</span>
              )}
            </div>
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
