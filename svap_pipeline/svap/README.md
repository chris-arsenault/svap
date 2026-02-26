# SVAP — Structural Vulnerability Analysis Pipeline

A six-stage analytical pipeline that identifies where policy structures are vulnerable to exploitation **before exploitation occurs at scale**. The system works backward from known enforcement cases, extracts abstract structural vulnerability qualities, then scans unevaluated policies for the same qualities and predicts what exploitation would emerge.

## How It Works

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│  Stage 1          Stage 2           Stage 3          Stage 4        │
│  ┌──────────┐     ┌──────────┐     ┌──────────┐    ┌──────────┐   │
│  │  Case     │────▶│ Taxonomy │────▶│Convergence│──▶│  Policy  │   │
│  │ Assembly  │     │Extraction│     │ Scoring   │   │ Scanning │   │
│  └──────────┘     └────┬─────┘     └─────┬─────┘   └────┬─────┘   │
│       ▲                │ ▲               │               │         │
│       │           ┌────┘ │          Validation           │         │
│    Documents    Human  Fail?          Gate              │         │
│    (RAG)       Review                                    ▼         │
│                                                    ┌──────────┐   │
│                          Stage 6              Stage 5│Exploit.  │   │
│                         ┌──────────┐    ┌──────────┐│Prediction│   │
│                         │Detection │◀───│  Human   ││          │   │
│                         │ Patterns │    │  Review  │└──────────┘   │
│                         └──────────┘    └──────────┘               │
│                                                                     │
│  ── All intermediate outputs stored in SQLite for resumability ──   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Quick Start

### 1. Install Dependencies

```bash
pip install -r requirements.txt
```

### 2. Configure

Edit `config.yaml` with your AWS Bedrock settings:

```yaml
bedrock:
  region: us-east-1
  model_id: anthropic.claude-sonnet-4-20250514-v1:0
  max_tokens: 4096

storage:
  db_path: ./svap_data.db

rag:
  chunk_size: 1500
  chunk_overlap: 200
  max_context_chunks: 10
```

### 3. Seed with Example Data (Optional)

To replicate the HHS OIG healthcare fraud analysis:

```bash
python -m svap.orchestrator seed
```

This loads the example enforcement cases, taxonomy, and policies from `svap/examples/`.

### 4. Run a Stage

```bash
# Run a single stage
python -m svap.orchestrator run --stage 1

# Run all stages sequentially
python -m svap.orchestrator run --stage all

# Resume from where you left off
python -m svap.orchestrator run --stage 4  # picks up stored Stage 3 outputs

# View current pipeline state
python -m svap.orchestrator status
```

### 5. Export Results

```bash
python -m svap.orchestrator export --format markdown --output results/
python -m svap.orchestrator export --format json --output results/
```

## Project Structure

```
svap/
├── README.md                          # This file
├── requirements.txt                   # Python dependencies
├── config.yaml                        # Pipeline configuration
├── svap/
│   ├── __init__.py
│   ├── orchestrator.py                # Main pipeline runner & CLI
│   ├── storage.py                     # SQLite persistence layer
│   ├── bedrock_client.py              # AWS Bedrock API wrapper
│   ├── rag.py                         # Document chunking & retrieval
│   ├── stages/
│   │   ├── __init__.py
│   │   ├── stage1_case_assembly.py    # Enforcement case extraction
│   │   ├── stage2_taxonomy.py         # Vulnerability quality extraction
│   │   ├── stage3_scoring.py          # Convergence scoring & calibration
│   │   ├── stage4_scanning.py         # Policy corpus scanning
│   │   ├── stage5_prediction.py       # Exploitation prediction
│   │   └── stage6_detection.py        # Detection pattern generation
│   ├── prompts/                       # Prompt templates (editable)
│   │   ├── stage1_extract.txt
│   │   ├── stage2_cluster.txt
│   │   ├── stage2_refine.txt
│   │   ├── stage3_score.txt
│   │   ├── stage4_characterize.txt
│   │   ├── stage4_score.txt
│   │   ├── stage5_predict.txt
│   │   └── stage6_detect.txt
│   └── examples/                      # Seed data for HHS OIG replication
│       ├── seed_cases.json
│       ├── seed_taxonomy.json
│       └── seed_policies.json
└── docs/
    ├── REPLICATION_GUIDE.md           # Step-by-step HHS OIG replication
    ├── ARCHITECTURE.md                # Technical architecture details
    ├── PROMPT_ENGINEERING.md          # Guide to customizing prompts
    └── DATA_MODEL.md                  # Database schema documentation
```

## Key Design Decisions

- **SQLite for storage**: No infrastructure dependencies. The entire pipeline state lives in one portable file. Swap for PostgreSQL/DynamoDB if you need multi-user access.
- **Prompt templates as files**: Every prompt is an editable `.txt` file with `{variable}` placeholders. Iterate on prompts without touching code.
- **Stage isolation**: Each stage reads from and writes to the database. You can re-run any stage independently. Outputs from later stages don't affect earlier ones.
- **Human gates**: Stages 2 and 5 have mandatory review steps. The pipeline pauses and exports review artifacts; resume after review with `--approve`.

## Documentation

| Document | Purpose |
|----------|---------|
| [REPLICATION_GUIDE.md](docs/REPLICATION_GUIDE.md) | Reproduce the HHS OIG analysis end-to-end |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design, data flow, extension points |
| [PROMPT_ENGINEERING.md](docs/PROMPT_ENGINEERING.md) | How to modify prompts for your domain |
| [DATA_MODEL.md](docs/DATA_MODEL.md) | Database schema and field definitions |
