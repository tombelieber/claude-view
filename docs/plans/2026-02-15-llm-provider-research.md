---
status: in-progress
date: 2026-02-15
companion-to: 2026-02-15-paid-llm-classification.md
---

# LLM Provider Research: Full-Session Classification at Scale

> **Goal**: Choose the best LLM provider for classifying ~3,000+ Claude Code sessions (170M tokens total) with full chat context. This is a living research document — updated as we evaluate providers and run benchmarks.

## To Human: Where We Left Off

**Date**: 2026-02-15
**Session**: Researched LLM providers for full-session classification. Did NOT write any code.

### What's Done

1. Scanned all 3,060 JSONL files (1.9GB) and measured real token counts — no more guessing
2. Discovered the current `preview` column is truncated to 200 chars (useless for quality classification)
3. Researched 25+ models across OpenRouter, Google, OpenAI, Anthropic, Mistral, DeepSeek, Zhipu, Moonshot, Alibaba
4. Computed full-session costs with context window constraints for every model
5. Identified Gemini 2.0 Flash as the front-runner ($12, 1M context, native JSON)

### What To Do Next (in priority order)

**Step 1: Quality Benchmark (most important, do this first)**
- Hand-label 50-100 sessions with ground-truth L1/L2/L3 categories
- Run the same sessions through 3-5 candidate models (Gemini Flash, DeepSeek V3, GPT-4o-mini, Mistral Small 3.2, GLM-4.6)
- Compare accuracy. If the $0.06 model matches the $0.10 model, cost difference is irrelevant
- This answers: "does cheap = good enough for our taxonomy?"

**Step 2: Implement `DirectApiProvider` for the winner**
- The `LlmProvider` trait already exists (`crates/core/src/llm/provider.rs`)
- Add a new provider that calls the HTTP API directly (no CLI spawn)
- Use `reqwest` + `tokio::sync::Semaphore` for concurrent requests
- This replaces the 15-20s/call CLI with ~300ms/call direct API

**Step 3: Fix the preview truncation**
- Change `discovery.rs:695` to store full first prompt (or at least 2000 chars)
- Better: for classification, read the full JSONL at classify-time instead of relying on the DB preview
- Decision: do we classify from DB summary or from raw JSONL? Raw is more flexible for future "insight analysis" use cases

**Step 4: Handle the fat tail (110 sessions > 200K tokens)**
- Option A: Just truncate at model's context limit (simple, lossy)
- Option B: Summarize-then-classify pipeline (better quality, more complex)
- Option C: Use Gemini 2.5 Pro (1M context, $1.25/M) only for these 110 sessions
- This is a $12 vs $60 decision, not a $12 vs $500 decision. Don't overthink it.

**Step 5: Add BYO API Key UI (Option C from paid-llm-classification.md)**
- Settings page: paste API key, select provider
- Encrypt key at rest in SQLite
- Frontend: provider selector dropdown, cost estimate before running

### Key Numbers to Remember

- **168M tokens** = your full dataset
- **$12** = cost to classify everything with Gemini Flash
- **1 minute** = wall-clock time at 50 concurrent (vs 17 hours with current CLI)
- **110 sessions** = the fat tail that breaks models with <200K context
- **43%** = how much of your token budget lives in those 110 sessions

### Files You'll Touch

| File | What |
|------|------|
| `crates/core/src/llm/provider.rs` | `LlmProvider` trait (already exists) |
| `crates/core/src/llm/` | New file: `gemini.rs` or `direct_api.rs` |
| `crates/core/src/llm/factory.rs` | Already exists, wire up new provider |
| `crates/core/src/classification.rs` | Batch prompt building, may need full-session variant |
| `crates/core/src/discovery.rs:695` | The 200-char truncation to fix |
| `crates/server/src/routes/classify.rs` | Bulk classification loop, add concurrency |

---

## Table of Contents

- [1. Our Dataset (Measured)](#1-our-dataset-measured)
- [2. Provider Comparison (Feb 2026)](#2-provider-comparison-feb-2026)
- [3. Cost Estimates: Full-Session, No Compromise](#3-cost-estimates-full-session-no-compromise)
- [4. Context Window Fit Analysis](#4-context-window-fit-analysis)
- [5. Recommendations](#5-recommendations)
- [6. Open Questions](#6-open-questions)
- [7. Sources](#7-sources)
- [Appendix A: Raw Data Scan](#appendix-a-raw-data-scan)
- [Appendix B: Cost Optimization Strategies](#appendix-b-cost-optimization-strategies)

---

## 1. Our Dataset (Measured)

Scanned on 2026-02-15 from `~/.claude/projects/` — **real data, not estimates**.

### Overall

| Metric | Value |
|--------|-------|
| Sessions | 3,060 |
| Total messages | 405,887 |
| Total content | 641 MB of text |
| Total tokens (est.) | **168.5M** |
| Sessions with first user prompt | 2,950 |

### Per-Session Token Distribution

| Percentile | Chars | Tokens (est.) |
|------------|-------|---------------|
| Median (P50) | 85,020 | 21,255 |
| Mean | 222,954 | 55,738 |
| P90 | 435,823 | 108,955 |
| P99 | 2,829,770 | 707,442 |
| Max | 28,534,295 | 7,133,573 |

### Session Size Buckets

| Bucket | Sessions | % of Sessions | Tokens | % of Total Tokens |
|--------|----------|---------------|--------|-------------------|
| <1K tokens | 676 | 22.1% | 0.3M | 0.2% |
| 1-10K | 517 | 16.9% | 2.5M | 1.5% |
| 10-50K | 1,045 | 34.2% | 29.6M | 17.6% |
| 50-128K | 603 | 19.7% | 46.2M | 27.4% |
| 128-200K | 109 | 3.6% | 16.9M | 10.0% |
| 200K-1M | 91 | 3.0% | 37.3M | 22.2% |
| >1M | 19 | 0.6% | 35.8M | 21.2% |

**Key insight**: 110 sessions (3.6%) contain **53M tokens (43% of all data)**. The fat tail dominates cost.

### First User Prompt Only (Current Implementation)

| Stat | Chars | Tokens |
|------|-------|--------|
| Median | 1,221 | 305 |
| Mean | 3,789 | 947 |
| P90 | 5,474 | 1,368 |
| P99 | 25,486 | 6,371 |
| Total | 11.2M | 2.8M |

**Current DB `preview` column**: Truncated to 200 chars (~50 tokens) in `discovery.rs:695`. This is inadequate for quality classification — we want full session context.

---

## 2. Provider Comparison (Feb 2026)

### Tier 1: Ultra-Cheap ($0.02–$0.15 / 1M input)

| Model | Input/1M | Output/1M | Context | JSON Output | Notes |
|-------|----------|-----------|---------|-------------|-------|
| **Mistral Nemo** | $0.02 | — | — | — | Cheapest available, unclear quality |
| **Gemma 3n E4B** | $0.03 | — | — | — | Cheapest on Artificial Analysis |
| **Mistral Small 3.1** | $0.03 | $0.11 | 128K | TBD | Very cheap, needs quality validation |
| **Mistral Small 3.2** | $0.06 | $0.18 | 128K | Improved | Better structured output than 3.1 |
| **DeepSeek V3** (MS provider) | $0.07 | $0.14 | 131K | Yes | Best value via OpenRouter |
| **Gemini 2.0 Flash** | $0.10 | $0.40 | **1M** | Native JSON Schema | Free tier available |
| **Gemini 2.0 Flash Lite** | $0.10 | $0.40 | **1M** | Native JSON Schema | Same pricing as Flash |
| **Yi-Lightning** | $0.14 | $0.14 | 16K | Yes | Flat rate, tiny context |
| **GPT-4o-mini** | $0.15 | $0.60 | 128K | Native JSON Schema | Most widely supported |

### Tier 2: Budget ($0.15–$0.80 / 1M input)

| Model | Input/1M | Output/1M | Context | JSON Output | Notes |
|-------|----------|-----------|---------|-------------|-------|
| **Qwen 2.5 72B** | $0.23 | $0.23 | 128K | Yes | GPT-4o quality at 1/10th cost |
| **DeepSeek V3.2** | $0.24-0.28 | $0.38-0.42 | 128K | Yes | Cache hit: $0.028/M (90% off) |
| **Gemini 2.5 Flash** | $0.30 | $2.50 | **1M** | Native JSON Schema | Explicitly optimized for classification |
| **GLM-4.5 Air** | $0.30 | $0.90 | — | Yes | Cheapest GLM option |
| **GLM-4.5 / 4.6** | $0.35 | $1.50 | 128K | Yes | Good multilingual |
| **Kimi K2.5** (Moonshot) | $0.45-0.60 | $2.25-2.50 | **262K** | Yes | Cache: $0.10-0.15/M |
| **Mistral Large 3** | $0.50 | $1.50 | **256K** | TBD | 675B params, huge context |

### Tier 3: Mid-Range ($0.80–$3.00 / 1M input)

| Model | Input/1M | Output/1M | Context | JSON Output | Notes |
|-------|----------|-----------|---------|-------------|-------|
| **Claude 3.5 Haiku** | $0.80 | $4.00 | 200K | Tool-based | Our current implementation |
| **GLM-5** | $0.80 | $2.56 | 200K | Yes | First open-weights model scoring 50+ on AA index |
| **Claude 4.5 Haiku** | $1.00 | $5.00 | 200K | Tool-based | Newer, slightly better |
| **Qwen 3 Max** | $1.20 | $6.00 | 252K | Yes | Tiered pricing |
| **Gemini 2.5 Pro** | $1.25 | $10.00 | **1M** | Native JSON Schema | Premium quality |

### Tier 4: Premium ($3.00+ / 1M input) — NOT for bulk classification

| Model | Input/1M | Output/1M |
|-------|----------|-----------|
| Claude 4.5 Sonnet | $3.00 | $15.00 |
| GPT-4.1 | $2.00 | $8.00 |
| GPT-4o | $2.50 | $10.00 |
| Claude Opus 4.5/4.6 | $5.00 | $25.00 |

### Free Options (OpenRouter)

| Model | Context | Notes |
|-------|---------|-------|
| **Gemini 2.0 Flash** (free tier) | 1M | 5-15 RPM, 100-1,000 req/day |
| **gpt-oss-120b** | — | 117B MoE, structured output |
| **Meta Llama 3.3 70B** | — | GPT-4 level performance |
| **Trinity Mini** (26B MoE) | 131K | Robust function calling |
| **Trinity Large Preview** (400B MoE) | 512K | 128K active |
| OpenRouter free router | varies | Auto-selects from free pool |

---

## 3. Cost Estimates: Full-Session, No Compromise

**Basis**: 170M input tokens + 300K output tokens (3,060 sessions, full chat, ~550 token system prompt each).

### Theoretical (Infinite Context)

| Model | Input Cost | Output Cost | **Total** |
|-------|-----------|-------------|-----------|
| DeepSeek V3 (MS) | $11.90 | $0.04 | **$11.94** |
| Gemini 2.0 Flash | $17.02 | $0.12 | **$17.14** |
| Mistral Small 3.2 | $10.21 | $0.05 | **$10.27** |
| GPT-4o-mini | $25.53 | $0.18 | **$25.72** |
| DeepSeek V3.2 | $47.66 | $0.13 | **$47.79** |
| Gemini 2.5 Flash | $51.06 | $0.75 | **$51.81** |
| Claude 3.5 Haiku | $136.17 | $1.22 | **$137.39** |
| Claude 3.5 Sonnet | $510.63 | $4.59 | **$515.22** |

### Realistic (With Context Limits)

Sessions exceeding context windows get truncated. This loses data from the most complex sessions.

| Model | Context | Sessions Truncated | Effective Tokens | **Total Cost** |
|-------|---------|-------------------|-----------------|----------------|
| **Gemini 2.0 Flash** | 1M | 19 (0.6%) | 153.4M | **$11.60** |
| **Mistral Large 3** | 256K | ~70 (2.3%) | ~135M | **~$10** |
| **Kimi K2.5** | 262K | ~70 (2.3%) | ~135M | **~$63** |
| **Claude 3.5 Haiku** | 200K | 110 (3.6%) | 119.0M | **$96.46** |
| **GPT-4o-mini** | 128K | 219 (7.2%) | 108.1M | **$16.41** |

### With Batch/Cache Discounts

| Model | Strategy | Effective Cost |
|-------|----------|---------------|
| DeepSeek V3.2 | Cache hit (90% off system prompt) | **~$45** |
| DeepSeek V3.2 | Off-peak hours (50% off) | **~$24** |
| OpenAI GPT-4o-mini | Batch API (50% off) | **~$13** |
| Gemini 2.0 Flash | Free tier (100-1000 req/day) | **$0** (but takes 3-30 days) |

---

## 4. Context Window Fit Analysis

This is the most important table. Models with small context windows lose data from the sessions that matter most.

| Model | Context | Sessions Exceeding | % Tokens Lost | Verdict |
|-------|---------|-------------------|---------------|---------|
| **Gemini 2.0/2.5 Flash** | 1M | 19 | 10.0% | Best fit |
| **Trinity Large** | 512K | ~30 | ~15% | Good |
| **Kimi K2.5** | 262K | ~70 | ~25% | OK |
| **Mistral Large 3** | 256K | ~70 | ~25% | OK |
| **Claude Haiku** | 200K | 110 | 30.4% | Loses a lot |
| **DeepSeek V3** | 131K | ~200 | ~35% | Loses the fat tail |
| **GPT-4o-mini** | 128K | 219 | 36.8% | Worst fit |
| **Yi-Lightning** | 16K | ~2,400 | ~90% | Useless for us |

**Critical insight**: 43% of our total tokens live in 110 sessions (>200K tokens each). Any model with <200K context loses the most analytically interesting sessions — the long, complex ones where classification context matters most.

---

## 5. Recommendations

### Primary: Gemini 2.0 Flash

| Factor | Score |
|--------|-------|
| Cost | $11.60 for all 3,060 sessions |
| Context fit | 99.4% of sessions fit untruncated |
| JSON output | Native JSON Schema support |
| Speed | ~1 min at 50 concurrent |
| Availability | Free tier exists, API well-documented |

### Runner-up: DeepSeek V3 via OpenRouter

| Factor | Score |
|--------|-------|
| Cost | $11.94 (cheapest overall) |
| Context fit | 131K — loses 35% of tokens |
| JSON output | Supported |
| Speed | Fast, good throughput |
| Concern | 128K context misses fat-tail sessions |

### For the 19 Monster Sessions (>1M tokens): Two-Phase Pipeline

1. **Phase 1**: Summarize with a large-context model (Gemini 2.5 Pro @ 1M context)
2. **Phase 2**: Classify the summary with the cheap model

Cost impact: 19 sessions × ~2M tokens × $1.25/M = ~$47 extra. Worth it for completeness.

### Models to Benchmark

Before committing, we should run a quality benchmark on 100 pre-labeled sessions:

| Priority | Model | Why |
|----------|-------|-----|
| 1 | Gemini 2.0 Flash | Best context + cost combo |
| 2 | DeepSeek V3.2 | Cheapest, 90% cache discount |
| 3 | Mistral Small 3.2 | Ultra-cheap, needs quality check |
| 4 | GPT-4o-mini | Baseline (most widely validated) |
| 5 | GLM-4.6 | Tops classification benchmarks per Stack AI |

### Models NOT Worth Pursuing

| Model | Why Not |
|-------|---------|
| Yi-Lightning | 16K context — useless for full-session |
| Gemma 3n E4B | Too small, quality concerns |
| Mistral Nemo | Unclear documentation, untested |
| Claude Sonnet/Opus | 5-50x more expensive, no quality justification for classification |
| Any reasoning model (R1, o1, o3) | Classification doesn't need chain-of-thought, wastes tokens |

---

## 6. Open Questions

- [ ] **Quality benchmark**: Which model actually classifies sessions best with our taxonomy? Need to hand-label 100 sessions and test.
- [ ] **OpenRouter vs direct API**: OpenRouter adds convenience but what's the markup? Some models are cheaper direct.
- [ ] **Prompt caching**: DeepSeek's 90% cache discount could be huge — our system prompt is identical across all 3,060 calls. Does Gemini offer similar?
- [ ] **Rate limits**: Can we actually do 50 concurrent requests? Need to check per-provider limits.
- [ ] **Beyond classification**: User wants "all kinds of interesting insight analysis" in the future. Does the chosen model handle open-ended analysis well, or only structured JSON?
- [ ] **Batch API availability**: OpenAI Batch API gives 50% off — does Gemini have an equivalent?
- [ ] **Self-hosting**: DeepSeek V3 is open-source. At what scale does self-hosting beat API costs?
- [ ] **Multi-model pipeline**: Route small sessions to a cheap model, large sessions to a large-context model?
- [ ] **Chinese model API stability**: GLM, Kimi, DeepSeek — are the APIs stable enough for production? Latency from outside China?

---

## 7. Sources

### Pricing Aggregators
- [OpenRouter Pricing](https://openrouter.ai/pricing)
- [PricePerToken.com](https://pricepertoken.com/) — Compare 300+ models
- [OpenRouter Model Comparison](https://compare-openrouter-models.pages.dev/)
- [InvertedStone Pricing Calculator](https://invertedstone.com/calculators/openrouter-pricing)

### Provider Official Pricing
- [Google Gemini API Pricing](https://ai.google.dev/gemini-api/docs/pricing)
- [OpenAI Pricing](https://openai.com/pricing)
- [Anthropic Claude Pricing](https://www.anthropic.com/pricing)
- [DeepSeek API Pricing](https://api-docs.deepseek.com/quick_start/pricing)
- [Moonshot/Kimi API Pricing](https://platform.moonshot.ai/docs/pricing/chat)
- [Alibaba Qwen Pricing](https://www.alibabacloud.com/help/en/model-studio/model-pricing)
- [Mistral AI Pricing](https://mistral.ai/products/pricing)

### Benchmarks & Leaderboards
- [LMSYS Chatbot Arena](https://arena.ai/leaderboard) — 5.27M votes, 305 models
- [Artificial Analysis LLM Leaderboard](https://artificialanalysis.ai/leaderboards/models)
- [Stack AI: Best LLMs by Task](https://www.stack-ai.com/blog/llm-leaderboard-which-llms-are-best-for-which-tasks)
- [WhatLLM Budget Analysis](https://whatllm.org/blog/best-budget-llms-january-2026)

### Structured Output Benchmarks
- [JSONSchemaBench](https://arxiv.org/abs/2501.10868) — 10K real-world JSON schemas
- [StructEval](https://arxiv.org/html/2505.20139v1) — 18 formats, 44 task types
- [Cleanlab Structured Output Benchmark](https://cleanlab.ai/blog/structured-output-benchmark/)

### Analysis & Comparison Articles
- [Gemini 2.5 Flash vs Claude 4.5 Haiku](https://www.appaca.ai/resources/llm-comparison/gemini-2.5-flash-vs-claude-4.5-haiku)
- [DeepSeek V3.2 Cost Advantage](https://introl.com/blog/deepseek-v3-2-open-source-ai-cost-advantage)
- [LLM Pricing Comparison 2026](https://www.cloudidr.com/blog/llm-pricing-comparison-2026)
- [Batch Processing for LLM Cost Savings](https://www.prompts.ai/blog/batch-processing-for-llm-cost-savings)

---

## Appendix A: Raw Data Scan

Scan performed 2026-02-15 on `/Users/TBGor/.claude/projects/`.

```
3,016 JSONL files, 1.9 GB total

File size distribution:
  <10KB:     650 files,   3,352 KB total
  10-100KB:  666 files,  29,636 KB total
  100KB-1MB: 1,384 files, 433,600 KB total
  1-10MB:    289 files, 880,508 KB total
  >10MB:     27 files, 650,540 KB total

Content breakdown:
  User messages:      513,635,609 chars (128.4M tokens)
  Assistant messages:  145,483,735 chars (36.4M tokens)
  Total content:      672,653,786 chars (168.2M tokens)
```

### First User Prompt Distribution

```
  2,950 sessions with identifiable first user prompts
  Median: 1,221 chars (305 tokens)
  Mean:   3,789 chars (947 tokens)
  P90:    5,474 chars (1,368 tokens)
  P99:   25,486 chars (6,371 tokens)
  Max:  788,301 chars (197,075 tokens)
  Total: 11,178,930 chars (2,794,732 tokens)
```

### Time Estimates (50 Concurrent Requests)

```
  Gemini 2.0 Flash:  ~64s  (1.1 min)
  GPT-4o-mini:       ~83s  (1.4 min)
  Claude 3.5 Haiku:  ~107s (1.8 min)
```

vs. current `claude -p` CLI approach: **~17 hours sequential** (15-20s per session).

---

## Appendix B: Cost Optimization Strategies

### 1. Prompt Caching

The system prompt (taxonomy + instructions, ~550 tokens) is identical for all 3,060 calls. Providers with prompt caching:

| Provider | Cache Discount | Effective System Prompt Cost |
|----------|---------------|------------------------------|
| DeepSeek | 90% off | $0.028/M instead of $0.28/M |
| Anthropic | 90% off | $0.08/M instead of $0.80/M |
| Kimi K2.5 | 75% off | $0.10-0.15/M |
| Google Gemini | Context caching available | TBD |

### 2. Batch APIs

| Provider | Batch Discount | Latency Trade-off |
|----------|---------------|-------------------|
| OpenAI | 50% off | Results in ~24 hours |
| Anthropic | 50% off (Message Batches) | Results in ~24 hours |

### 3. Off-Peak Pricing

| Provider | Discount | Hours (UTC) |
|----------|----------|-------------|
| DeepSeek | 50% off | 16:30–00:30 GMT |

### 4. Model Routing (Future Optimization)

Route sessions by size to optimize cost:

```
if session_tokens < 50K:
    use DeepSeek V3 ($0.07/M)     # 73% of sessions, 19% of tokens
elif session_tokens < 200K:
    use Gemini 2.0 Flash ($0.10/M) # 23% of sessions, 37% of tokens
else:
    use Gemini 2.5 Pro ($1.25/M)   # 3.6% of sessions, 43% of tokens
    # or summarize-then-classify pipeline
```

Estimated cost with routing: **~$7-8** vs $11.60 with Gemini Flash for everything.

---

*Last updated: 2026-02-15*
