import React, { useState } from "react";
import { usePipeline } from "../data/usePipelineData";
import { ScoreBar, QualityTags, RiskBadge } from "../components/SharedUI";
import type { ViewProps } from "../types";

interface CatalogNode {
  label: string;
  children?: Record<string, CatalogNode>;
  programs?: string[];
}

interface TreeNodeProps {
  nodeKey: string;
  node: CatalogNode;
  depth?: number;
  scannedPrograms?: string[];
}

function treeIcon(isExpandable: boolean, expanded: boolean): string {
  if (!isExpandable) return "\u00B7";
  return expanded ? "\u25BE" : "\u25B8";
}

const EMPTY_PROGRAMS: string[] = [];

function TreeNode({ node, depth = 0, scannedPrograms = EMPTY_PROGRAMS }: TreeNodeProps) {
  const [expanded, setExpanded] = useState(depth < 2);
  const isExpandable = !!node.children || !!node.programs;

  return (
    <div className={depth === 0 ? "tree-node root" : "tree-node"}>
      <div
        className="tree-label"
        role="button"
        tabIndex={0}
        onClick={() => setExpanded(!expanded)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setExpanded(!expanded);
          }
        }}
      >
        <span className="tree-icon">{treeIcon(isExpandable, expanded)}</span>
        <span
          style={{
            fontWeight: depth < 2 ? 600 : 400,
            color: depth === 0 ? "var(--text-primary)" : "var(--text-secondary)",
          }}
        >
          {node.label}
        </span>
        {node.programs && (
          <span style={{ fontSize: 10, color: "var(--text-muted)", marginLeft: "auto" }}>
            {node.programs.length} programs
          </span>
        )}
      </div>
      {expanded && <TreeChildren node={node} depth={depth} scannedPrograms={scannedPrograms} />}
    </div>
  );
}

function TreeChildren({
  node,
  depth,
  scannedPrograms,
}: {
  node: CatalogNode;
  depth: number;
  scannedPrograms: string[];
}) {
  return (
    <>
      {node.children &&
        Object.entries(node.children).map(([k, child]) => (
          <TreeNode
            key={k}
            nodeKey={k}
            node={child as CatalogNode}
            depth={depth + 1}
            scannedPrograms={scannedPrograms}
          />
        ))}
      {node.programs &&
        node.programs.map((prog) => {
          const isScanned = scannedPrograms.includes(prog);
          return (
            <div key={prog} className={isScanned ? "tree-leaf scanned" : "tree-leaf"}>
              {isScanned && <span style={{ marginRight: 4 }}>{"\u25C6"}</span>}
              {prog}
            </div>
          );
        })}
    </>
  );
}

interface DataSource {
  id: string;
  name: string;
}

interface DataSourceCategory {
  label: string;
  sources: DataSource[];
}

function ScanResultsTable({
  policies,
  threshold,
}: {
  policies: { policy_id: string; name: string; convergence_score: number; risk_level: string; qualities: string[] }[];
  threshold: number;
}) {
  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Scan Results</h3>
      </div>
      <div className="panel-body dense">
        <table className="data-table">
          <thead>
            <tr>
              <th>Policy</th>
              <th>Score</th>
              <th>Risk</th>
              <th>Qualities</th>
            </tr>
          </thead>
          <tbody>
            {[...policies]
              .sort((a, b) => b.convergence_score - a.convergence_score)
              .map((p) => (
                <tr key={p.policy_id}>
                  <td style={{ fontWeight: 500 }}>{p.name}</td>
                  <td>
                    <ScoreBar score={p.convergence_score} threshold={threshold} />
                  </td>
                  <td>
                    <RiskBadge level={p.risk_level as "critical" | "high" | "medium" | "low"} />
                  </td>
                  <td>
                    <QualityTags ids={p.qualities} />
                  </td>
                </tr>
              ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function DataSourcesPanel({ dataSources }: { dataSources: Record<string, unknown> }) {
  return (
    <div className="panel stagger-in">
      <div className="panel-header">
        <h3>Available Data Sources</h3>
      </div>
      <div className="panel-body">
        {Object.entries(dataSources).map(([catKey, cat]) => {
          const category = cat as DataSourceCategory;
          return (
            <div key={catKey} style={{ marginBottom: 16 }}>
              <div
                style={{
                  fontFamily: "var(--font-display)",
                  fontSize: 11,
                  fontWeight: 600,
                  textTransform: "uppercase",
                  letterSpacing: "0.06em",
                  color: "var(--text-muted)",
                  marginBottom: 6,
                }}
              >
                {category.label}
              </div>
              {category.sources.map((s) => (
                <div key={s.id} style={{ fontSize: 12, padding: "4px 0", color: "var(--text-secondary)" }}>
                  <span style={{ color: "var(--accent-bright)", fontFamily: "var(--font-mono)", fontSize: 11 }}>
                    {s.id}
                  </span>
                  {" \u2014 "}
                  {s.name}
                </div>
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default function PolicyExplorer({ onNavigate: _onNavigate }: ViewProps) {
  const { policies, policy_catalog, scanned_programs, data_sources, threshold } = usePipeline();

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Policy Explorer</h2>
        <div className="view-desc">
          HHS policy catalog — <span style={{ color: "var(--accent-bright)" }}>{"\u25C6"} scanned policies</span> have
          been evaluated against the vulnerability taxonomy
        </div>
      </div>

      <div className="split-view">
        <div className="panel stagger-in">
          <div className="panel-header">
            <h3>HHS Policy Catalog</h3>
            <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{scanned_programs.length} scanned</span>
          </div>
          <div className="panel-body" style={{ maxHeight: 600, overflowY: "auto" }}>
            {Object.entries(policy_catalog).map(([k, node]) => (
              <TreeNode key={k} nodeKey={k} node={node as CatalogNode} depth={0} scannedPrograms={scanned_programs} />
            ))}
          </div>
        </div>

        <div>
          <ScanResultsTable policies={policies} threshold={threshold} />
          <DataSourcesPanel dataSources={data_sources} />
        </div>
      </div>
    </div>
  );
}
