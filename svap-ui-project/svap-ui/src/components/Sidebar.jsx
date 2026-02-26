import React from 'react';
import {
  LayoutDashboard, FileSearch, FolderTree, Tags, Grid,
  AlertTriangle, Radio, Shield
} from 'lucide-react';
import { StageDot } from './SharedUI';

const NAV_ITEMS = [
  { section: 'Analysis' },
  { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
  { id: 'cases', label: 'Case Sourcing', icon: FileSearch, badge: '8' },
  { id: 'policies', label: 'Policy Explorer', icon: FolderTree },
  { section: 'Pipeline Results' },
  { id: 'taxonomy', label: 'Taxonomy', icon: Tags, badge: '8' },
  { id: 'matrix', label: 'Convergence Matrix', icon: Grid },
  { id: 'predictions', label: 'Predictions', icon: AlertTriangle, badge: '4' },
  { id: 'detection', label: 'Detection Patterns', icon: Radio, badge: '6' },
];

const PIPELINE_STAGES = [
  { num: 1, name: 'Case Assembly', status: 'completed' },
  { num: 2, name: 'Taxonomy Extraction', status: 'completed' },
  { num: 3, name: 'Convergence Scoring', status: 'completed' },
  { num: 4, name: 'Policy Scanning', status: 'completed' },
  { num: 5, name: 'Exploitation Prediction', status: 'completed' },
  { num: 6, name: 'Detection Patterns', status: 'completed' },
];

export default function Sidebar({ activeView, onNavigate }) {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <h1>SVAP</h1>
        <div className="subtitle">HHS OIG Workstation</div>
      </div>

      <nav className="sidebar-nav">
        {NAV_ITEMS.map((item, i) => {
          if (item.section) {
            return <div key={i} className="nav-section-label">{item.section}</div>;
          }
          const Icon = item.icon;
          return (
            <div
              key={item.id}
              className={`nav-item ${activeView === item.id ? 'active' : ''}`}
              onClick={() => onNavigate(item.id)}
            >
              <Icon />
              <span>{item.label}</span>
              {item.badge && <span className="nav-badge">{item.badge}</span>}
            </div>
          );
        })}
      </nav>

      <div className="pipeline-status">
        <div className="nav-section-label" style={{ padding: '0 0 8px' }}>Pipeline Status</div>
        {PIPELINE_STAGES.map(s => (
          <div key={s.num} className="stage-row">
            <StageDot status={s.status} />
            <span style={{ color: 'var(--text-secondary)', fontSize: 11 }}>
              {s.num}. {s.name}
            </span>
          </div>
        ))}
      </div>
    </aside>
  );
}
