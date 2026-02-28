import React from "react";
import { ChevronDown, ChevronRight, GitBranch } from "lucide-react";
import { useExploitationTrees } from "../data/usePipelineSelectors";
import { useExpandSingle, expandableProps } from "../hooks";
import { QualityTags, ScoreBar, Badge, ViewHeader } from "../components/SharedUI";
import type { ExploitationTree, ExploitationStep } from "../types";

const difficultyLevel = (d?: string): "critical" | "high" | "medium" | "neutral" => {
  if (!d) return "neutral";
  const lower = d.toLowerCase();
  if (lower.includes("hard")) return "critical";
  if (lower.includes("medium")) return "high";
  return "medium";
};

function StepDetail({ step }: { step: ExploitationStep }) {
  return (
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
  );
}

function StepHeader({ step, isExpanded }: { step: ExploitationStep; isExpanded: boolean }) {
  return (
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
  );
}

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
  const hasDetail = [step.description, step.actor_action, step.enabling_qualities?.length].some(Boolean);

  return (
    <div className="tree-step" style={{ '--step-depth': depth } as React.CSSProperties}>
      <div
        className={`tree-step-content${hasDetail ? " clickable" : ""}`}
        {...(hasDetail ? expandableProps(() => onToggle(step.step_id)) : {})}
      >
        <StepHeader step={step} isExpanded={isExpanded} />
        {isExpanded && <StepDetail step={step} />}
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
  const { expandedId: expandedStep, toggle: toggleStep } = useExpandSingle();

  return (
    <div className="panel stagger-in">
      <div
        className="panel-header clickable"
        {...expandableProps(() => onToggle(tree.tree_id))}
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
  const trees = useExploitationTrees();
  const { expandedId: expandedTree, toggle } = useExpandSingle();

  return (
    <div>
      <ViewHeader
        title="Exploitation Trees"
        description={<>{trees.length} exploitation trees — each models a branching attack pathway for a high-risk policy, with steps linked to enabling qualities</>}
      />

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
