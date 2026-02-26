# Replication Guide: Reproducing the HHS OIG Healthcare Fraud Analysis

This guide walks you through reproducing the structural vulnerability analysis of HHS policy areas that identified HCBS, PACE, and Hospital-at-Home as the highest-risk targets for proactive fraud investigation.

## What You're Replicating

The original analysis:
1. Extracted 8 known fraud schemes from 2024–2025 HHS OIG/DOJ enforcement actions
2. Decomposed each scheme into the structural policy properties that enabled it
3. Identified 8 abstract vulnerability qualities (V1–V8) that recur across schemes
4. Scored known cases against the taxonomy, finding that ≥3 converging qualities predict large-scale ($500M+) exploitation
5. Scanned 7 unevaluated HHS policy areas and predicted which would be most vulnerable
6. Found HCBS (score=6), PACE (score=5), and Hospital-at-Home (score=5) as highest-risk

## Prerequisites

- Python 3.10+
- AWS account with Bedrock access to Claude (Sonnet or better)
- AWS credentials configured (`~/.aws/credentials` or environment variables)

## Step-by-Step Replication

### Step 1: Setup

```bash
cd svap/
pip install -r requirements.txt

# Edit config.yaml with your Bedrock region and model ID
cp config.yaml config.yaml.backup
# Edit config.yaml — at minimum set bedrock.region and bedrock.model_id
```

### Step 2: Seed the Example Data

This loads the pre-extracted enforcement cases and vulnerability taxonomy so you can start from a validated foundation rather than re-extracting from raw documents.

```bash
python -m svap.orchestrator seed
```

This creates:
- 8 enforcement cases (Stage 1 output) in the database
- 8 vulnerability qualities (Stage 2 output), pre-approved
- 7 policy areas to scan (Stage 4 input)

### Step 3: Run Convergence Scoring (Stage 3)

This scores each known case against the taxonomy and calibrates the threshold.

```bash
python -m svap.orchestrator run --stage 3
```

Expected output: A threshold of approximately 3, with all $500M+ cases scoring ≥3. If your results differ significantly, the model may be interpreting the taxonomy differently — check the evidence fields in the convergence matrix.

### Step 4: Run Policy Scanning (Stage 4)

This characterizes each unevaluated policy's structural properties and scores it.

```bash
python -m svap.orchestrator run --stage 4
```

Expected output: HCBS should score highest (5–6), followed by PACE and AHCAH (4–5). The exact scores may vary by ±1 depending on how the model interprets the structural characterizations.

### Step 5: Generate Exploitation Predictions (Stage 5)

```bash
python -m svap.orchestrator run --stage 5
```

This generates predictions for all policies scoring at or above the threshold. Review the predictions — they should be structurally entailed by the vulnerability qualities, not speculative.

### Step 6: Review and Approve Predictions

```bash
python -m svap.orchestrator export --stage 5 --format markdown
# Review the exported predictions
python -m svap.orchestrator approve --stage 5
```

### Step 7: Generate Detection Patterns (Stage 6)

```bash
python -m svap.orchestrator run --stage 6
```

### Step 8: Export Full Results

```bash
python -m svap.orchestrator export --format markdown --output ./results/
python -m svap.orchestrator export --format json --output ./results/
```

## Extending with Your Own Data

### Adding Your Own Enforcement Cases

If you have access to internal enforcement data (OIG case files, MFCU referrals, audit findings):

```bash
# Ingest enforcement documents
python -m svap.orchestrator ingest --path /path/to/enforcement/docs --type enforcement

# Re-run Stage 1 to extract cases from new documents
python -m svap.orchestrator run --stage 1
```

### Adding Your Own Policies

If you want to scan additional policy areas:

1. Create a JSON file following the format in `svap/examples/seed_policies.json`
2. Load it manually, or ingest policy documents:

```bash
python -m svap.orchestrator ingest --path /path/to/policy/manuals --type policy
```

### Refining the Taxonomy

The taxonomy is the most important asset. If your internal data reveals enabling conditions not captured by V1–V8:

1. Add new enforcement cases (Step above)
2. Re-run Stage 2 to re-cluster enabling conditions
3. The new taxonomy may have 9–10 qualities instead of 8
4. Re-run Stages 3–6 with the new taxonomy

## Connecting to Internal Data Sources

The seed data uses publicly available enforcement actions. In your internal environment, you can dramatically improve results by connecting to:

- **CMS claims databases** → feed actual claims data into the policy characterization prompts for Stage 4
- **Provider enrollment files** → improve the accuracy of V8 (Low Barriers) scoring
- **OIG audit reports and work papers** → much richer case data than public press releases for Stage 1
- **State MFCU referrals** → cases that never reached public prosecution but reveal patterns
- **CMS program integrity data** → improper payment rates, denial rates, and audit findings that validate convergence scoring

The RAG system supports ingesting these as documents. For structured data (claims databases), you would add a data extraction step that summarizes relevant statistics into text that can be included in prompt context.

## Validation Checklist

After replication, verify:

- [ ] Convergence scores for known cases correlate with scale (higher score → larger scheme)
- [ ] Threshold separates minor and major exploitation (typically ≥3)
- [ ] No known major case ($500M+) scores below threshold
- [ ] Policy scores are robust: re-running with temperature=0 produces consistent results
- [ ] Predictions cite specific enabling qualities (no free-form speculation)
- [ ] Detection patterns specify concrete data sources and thresholds
