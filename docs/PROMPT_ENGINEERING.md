# Prompt Engineering Guide

All prompts live in `svap/prompts/` as plain `.txt` files with `{variable}` placeholders. You can modify them without touching any code.

## General Principles

### Temperature by Stage
- **Stages 1, 3, 4 (extraction/scoring):** Low temperature (0.1–0.2). These tasks have correct answers — you want deterministic outputs.
- **Stage 2 (taxonomy):** Medium temperature (0.2–0.3). Abstraction benefits from slight creativity, but too much produces inconsistent quality names.
- **Stage 5 (prediction):** Slightly higher (0.3). Predictions should be creative but constrained.
- **Stage 6 (detection):** Low-medium (0.2). Detection patterns should be precise and actionable.

### Structured Output Enforcement
Every prompt that needs parseable output ends with a JSON schema specification. Key patterns:

1. **"Return ONLY valid JSON"** — critical instruction. Without it, models tend to add preamble text.
2. **Schema example** — showing the exact JSON structure reduces format errors by ~90% vs. describing it in prose.
3. **Field-level instructions** — put constraints in the schema description, not just in the task instructions.

### The Constraint Cascade
The most important prompt engineering pattern in this pipeline is the **constraint cascade** — each stage's output is constrained by the previous stage's output:

- Stage 2 qualities are constrained to match Stage 1 enabling conditions
- Stage 3 scores are constrained to the Stage 2 recognition tests
- Stage 5 predictions are constrained to cite Stage 2 qualities
- Stage 6 patterns are constrained to address Stage 5 predictions

This constraint cascade is what prevents the pipeline from producing generic, unfounded outputs. **Never loosen these constraints.**

## Stage-Specific Prompt Guidance

### Stage 1: `stage1_extract.txt`
**Key challenge:** Getting the model to identify the STRUCTURAL enabling condition, not just restating "there was fraud."

The most common failure mode is the model writing enabling conditions like "weak oversight" or "insufficient monitoring." The system prompt explicitly forbids this, and the prompt gives counter-examples. If you still get vague enabling conditions, add more negative examples:

```
BAD enabling condition: "Weak oversight allowed fraud to occur"
BAD enabling condition: "Insufficient monitoring"
GOOD enabling condition: "Payment issued before independent verification of service delivery"
GOOD enabling condition: "Provider self-reports the diagnosis codes that determine its own risk-adjusted payment"
```

### Stage 2: `stage2_cluster.txt` and `stage2_refine.txt`
**Key challenge:** Getting the right level of abstraction — specific enough to be useful, general enough to transfer across policies.

If qualities are too specific (e.g., "Medicare FFS pays before audit"), they won't transfer to other policies. If too abstract (e.g., "the system has flaws"), they're useless. The target is the level of "Payment Precedes Verification" — specific enough to apply a recognition test, general enough to apply to any payment system.

The refinement prompt's **independence check** is critical. If two qualities always co-occur, the taxonomy is over-specified. Common merge candidates:
- "Low provider barriers" and "Rapid enrollment expansion" might merge into a single "Permeable entry" quality
- "Subjective criteria" and "Self-attesting basis" might merge (but usually shouldn't — they address different structural dimensions)

### Stage 3: `stage3_score.txt`
**Key challenge:** Preventing over-scoring. The model tends to be generous — marking qualities as PRESENT when they're only weakly suggested.

Counter this with:
- "Mark ABSENT if ambiguous" (explicit in the prompt)
- Requiring one-sentence evidence for every score
- Low temperature (0.1–0.2)

If validation shows that convergence scores don't correlate with exploitation severity, over-scoring is the most likely cause. Tighten the recognition tests in Stage 2 before re-running Stage 3.

### Stage 4: `stage4_characterize.txt` and `stage4_score.txt`
**Key challenge:** Getting enough structural detail in the characterization to support accurate scoring.

The characterization prompt asks for seven specific structural dimensions (payment model, verification, eligibility controls, clinical criteria, service visibility, change velocity, intermediary structure). If any dimension is missing from the characterization, the scoring prompt will correctly mark related qualities as ABSENT — but this may be a false negative caused by incomplete characterization, not by the quality truly being absent.

**Fix:** If a policy scores lower than expected, check whether the characterization addresses all seven dimensions. If not, you can:
1. Provide more context in the policy description
2. Ingest the actual policy document into RAG so the characterization prompt has more source material
3. Manually supplement the characterization

### Stage 5: `stage5_predict.txt`
**Key challenge:** Keeping predictions structurally grounded rather than generic.

The prompt's most important constraint: "Every predicted step must be CAUSED by a specific vulnerability quality or combination. If you cannot cite the enabling quality, REMOVE the step."

If predictions start sounding like generic fraud descriptions rather than structurally specific ones, the constraint isn't biting hard enough. Try adding:

```
ANTI-PATTERN: "Providers will submit false claims" — this is generic 
and doesn't cite a specific quality. REMOVE predictions like this.

GOOD PATTERN: "Because this policy has V1 (payment precedes verification) 
AND V8 (low barriers), an organized network can rapidly establish multiple 
billing entities, submit large volumes of claims, collect payment before 
any audit occurs, then dissolve the entities." — this cites V1+V8 and 
explains the interaction.
```

### Stage 6: `stage6_detect.txt`
**Key challenge:** Generating patterns specific enough to implement, not so specific they miss real fraud.

The prompt gives examples of GOOD vs. BAD anomaly signals. The key is quantitative thresholds and comparison baselines:

```
BAD: "Look for unusual billing patterns"
GOOD: "Flag providers billing >16 hours/day of personal care, where P95 is 10 hours/day"
```

If your team has access to actual data dictionaries and table schemas, inject them into the `{data_sources}` variable. The more specific the model can be about which table, which column, and which join condition, the more directly implementable the output becomes.

## Testing Prompts

Before deploying prompt changes:

1. **Run on seed data**: Re-run the full pipeline on seed data and compare outputs to the reference analysis
2. **Check constraint adherence**: Verify that Stage 5 predictions all cite qualities, Stage 6 patterns all have thresholds
3. **Spot-check reasoning**: Read 3–5 outputs per stage. Are the evidence fields coherent? Do scores match your expert judgment?
4. **Temperature sensitivity**: Run the same stage twice at the same temperature. Outputs should be substantively similar (exact wording will differ, but scores and conclusions should be stable)
