import React, { useCallback, useState } from "react";
import { ChevronDown, ChevronRight, GitBranch } from "lucide-react";
import { usePipelineStore } from "../data/pipelineStore";
import { QualityTags, ScoreBar, Badge } from "../components/SharedUI";
import type { ExploitationTree, ExploitationStep } from "../types";

const difficultyLevel = (d?: string): "critical" | "high" | "medium" | "neutral" => {
  if (!d) return "neutral";
  const lower = d.toLowerCase();
  if (lower.includes("hard")) return "critical";
  if (lower.includes("medium")) return "high";
  return "medium";
};

function StepNode({
  step,
  depth,
  isExpanded,
  onToggle,
}: {
  step: ExploitationStep;
  depth: number;
  isExpanded: boolean;
  onToggle: (id: string) => void;
}) {
  const hasDetail = step.description || step.actor_action || step.enabling_qualities?.length > 0;
  return (
    <div className="tree-step" style={{ paddingLeft: `${depth * 24 + 8}px` }}>
      <div
        className={`tree-step-content${hasDetail ? " clickable" : ""}`}
        role={hasDetail ? "button" : undefined}
        tabIndex={hasDetail ? 0 : undefined}
        onClick={hasDetail ? () => onToggle(step.step_id) : undefined}
        onKeyDown={hasDetail ? (e) => {
          if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onToggle(step.step_id); }
        } : undefined}
      >
        <div className="tree-step-header">
          <span className="tree-step-order">{step.step_order}</span>
          <span className="tree-step-title">{step.title}</span>
          {step.is_branch_point && (
            <span className="tree-branch-badge">
              <GitBranch size={12} /> branch
            </span>
          )}
          {step.branch_label && (
            <Badge level="neutral">{step.branch_label}</Badge>
          )}
          {step.enabling_qualities?.length > 0 && !isExpanded && (
            <span className="tree-step-qual-count">
              {step.enabling_qualities.length}q
            </span>
          )}
        </div>
        {isExpanded && (
          <div className="tree-step-detail">
            {step.description && (
              <div className="tree-step-desc">{step.description}</div>
            )}
            {step.actor_action && (
              <div className="tree-step-actor">{step.actor_action}</div>
            )}
            {step.enabling_qualities?.length > 0 && (
              <div className="tree-step-qualities">
                <QualityTags ids={step.enabling_qualities} />
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function buildStepTree(steps: ExploitationStep[]): { step: ExploitationStep; depth: number }[] {
  const result: { step: ExploitationStep; depth: number }[] = [];
  const depthMap = new Map<string, number>();

  for (const step of steps) {
    const parentDepth = step.parent_step_id ? (depthMap.get(step.parent_step_id) ?? 0) : 0;
    const depth = step.parent_step_id ? parentDepth + 1 : 0;
    depthMap.set(step.step_id, depth);
    result.push({ step, depth });
  }
  return result;
}

function TreeCard({
  tree,
  isExpanded,
  onToggle,
}: {
  tree: ExploitationTree;
  isExpanded: boolean;
  onToggle: (id: string) => void;
}) {
  const stepTree = buildStepTree(tree.steps || []);
  const branchCount = (tree.steps || []).filter((s) => s.is_branch_point).length;
  const [expandedStep, setExpandedStep] = useState<string | null>(null);
  const toggleStep = useCallback(
    (id: string) => setExpandedStep((prev) => (prev === id ? null : id)),
    [],
  );

  return (
    <div className="panel stagger-in">
      <div
        className="panel-header clickable"
        role="button"
        tabIndex={0}
        onClick={() => onToggle(tree.tree_id)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onToggle(tree.tree_id);
          }
        }}
      >
        <div className="prediction-header-left">
          {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          <h3 className="prediction-policy-name">{tree.policy_name}</h3>
        </div>
        <div className="prediction-header-right">
          <ScoreBar score={tree.convergence_score} threshold={3} />
          <Badge level={difficultyLevel(tree.detection_difficulty)}>
            {tree.detection_difficulty?.split("\u2014")[0]?.trim() || "Unknown"}
          </Badge>
          <span className="tree-header-stat">{tree.step_count} steps</span>
          {branchCount > 0 && (
            <span className="tree-header-stat">{branchCount} branches</span>
          )}
        </div>
      </div>

      {isExpanded && (
        <div className="panel-body panel-body-bordered">
          <div className="tree-meta-row">
            <span className="tree-meta-item">
              <span className="tree-meta-label">Actor</span> {tree.actor_profile}
            </span>
            <span className="tree-meta-item">
              <span className="tree-meta-label">Lifecycle</span> {tree.lifecycle_stage}
            </span>
          </div>
          <div className="tree-steps">
            {stepTree.map(({ step, depth }) => (
              <StepNode
                key={step.step_id}
                step={step}
                depth={depth}
                isExpanded={expandedStep === step.step_id}
                onToggle={toggleStep}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export default function PredictionView() {
  const trees = usePipelineStore((s) => s.exploitation_trees);
  const [expandedTree, setExpandedTree] = useState<string | null>(null);
  const toggle = useCallback(
    (id: string) => setExpandedTree((prev) => (prev === id ? null : id)),
    [],
  );

  return (
    <div>
      <div className="view-header stagger-in">
        <h2>Exploitation Trees</h2>
        <div className="view-desc">
          {trees.length} exploitation trees — each models a branching attack pathway for a
          high-risk policy, with steps linked to enabling qualities
        </div>
      </div>

      {trees.map((tree) => (
        <TreeCard
          key={tree.tree_id}
          tree={tree}
          isExpanded={expandedTree === tree.tree_id}
          onToggle={toggle}
        />
      ))}
    </div>
  );
}
