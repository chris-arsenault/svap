# Replication Guide

Step-by-step guide to reproducing the structural vulnerability analysis of HHS policy areas.

## What You're Replicating

The original analysis:
1. Extracted known fraud schemes from HHS OIG/DOJ enforcement actions
2. Decomposed each scheme into the structural policy properties that enabled it
3. Identified abstract vulnerability qualities that recur across schemes
4. Scored known cases against the taxonomy, finding that converging qualities predict large-scale exploitation
5. Scanned unevaluated HHS policy areas for the same structural patterns
6. Generated exploitation predictions and detection strategies for the highest-risk policies

## Prerequisites

- Python 3.12+
- AWS account with Bedrock access to Claude (Sonnet or better)
- AWS credentials configured (`~/.aws/credentials` or environment variables)
- PostgreSQL database (local or RDS)

## Step-by-Step Replication

### Step 1: Setup

```bash
cd backend
uv sync
cp config.yaml config.local.yaml   # Edit DATABASE_URL for your environment
```

### Step 2: Seed the Reference Data

Load the pre-curated enforcement sources, taxonomy, and policy catalog so you start from a validated foundation.

```bash
make seed
```

This creates:
- 8 enforcement sources with URLs to public enforcement documents
- 8 source feeds for document discovery
- 8 vulnerability qualities (pre-approved taxonomy)
- 7 policy areas to scan

### Step 3: Run the Full Pipeline

```bash
# Run all stages sequentially
uv run -m svap.orchestrator run --stage all

# Or run stages individually
uv run -m svap.orchestrator run --stage 0   # Fetch documents
uv run -m svap.orchestrator run --stage 1   # Extract cases
uv run -m svap.orchestrator run --stage 2   # Extract/refine taxonomy
```

Stage 2 will detect the seeded cases as new, cluster their enabling conditions, and semantically deduplicate the results against the seed taxonomy. Most extracted qualities should merge with existing seed entries, enriching them with new canonical examples.

### Step 4: Review Taxonomy (Human Gate)

If Stage 2 produces novel qualities not covered by the seed taxonomy, the pipeline pauses for review.

```bash
uv run -m svap.orchestrator status          # Check pipeline state
uv run -m svap.orchestrator approve --stage 2
```

If all extracted qualities merged with existing taxonomy entries, Stage 2 completes automatically.

### Step 5: Continue Through Remaining Stages

```bash
uv run -m svap.orchestrator run --stage 3   # Convergence scoring
uv run -m svap.orchestrator run --stage 4   # Policy scanning
uv run -m svap.orchestrator run --stage 5   # Predictions
uv run -m svap.orchestrator approve --stage 5
uv run -m svap.orchestrator run --stage 6   # Detection patterns
```

### Step 6: Review Results

```bash
uv run -m svap.orchestrator status
```

Or start the frontend to explore results interactively:

```bash
cd frontend && npm run dev
```

## Extending with Your Own Data

### Adding Enforcement Cases

If you have access to internal enforcement data (OIG case files, MFCU referrals, audit findings):

1. Add documents as enforcement sources via the UI or API
2. Re-run Stages 0-1 to fetch and extract new cases
3. Re-run Stage 2 -- it will process only the new cases and deduplicate against the existing taxonomy

### Adding Policies

Add policies to the catalog via the seed data or API, then re-run Stage 4 to scan them.

### Refining the Taxonomy

The taxonomy is the pipeline's most important asset. As new enforcement data reveals enabling conditions not captured by existing qualities:

1. Add new enforcement documents and re-run Stages 0-2
2. Stage 2's delta processing extracts from new cases only
3. Semantic deduplication merges overlapping qualities automatically
4. Novel qualities are flagged for human review before downstream stages use them
5. Re-run Stages 3-6 with the expanded taxonomy

### Connecting to Internal Data Sources

The seed data uses publicly available enforcement actions. Internal environments can improve results by connecting to:

- **Claims databases** -- feed actual claims data into Stage 4 policy characterization prompts
- **Provider enrollment files** -- improve accuracy of barrier-related quality scoring
- **OIG audit reports** -- richer case data than public press releases for Stage 1
- **State MFCU referrals** -- cases that never reached public prosecution but reveal patterns
- **Program integrity data** -- improper payment rates, denial rates, and audit findings

The RAG system supports ingesting these as documents. For structured data, add an extraction step that summarizes relevant statistics into text for prompt context.

## Validation Checklist

After replication, verify:

- [ ] Convergence scores for known cases correlate with scale (higher score = larger scheme)
- [ ] Calibration threshold separates minor and major exploitation
- [ ] No known major case scores below threshold
- [ ] Policy scores are robust: re-running produces consistent results
- [ ] Predictions cite specific enabling qualities (no free-form speculation)
- [ ] Detection patterns specify concrete data sources and thresholds
