# 2026-04-16 — Recent literature check (2023+ SOTA)

## Why

The mentor's 2026-04-14 email asks us to "compare against state-of-the-art" with benchmarks committed to the repo. Yesterday's work (φ-accrual 2004, Satzger 2007) covers the canonical statistical detectors. Today's question: is there a 2023–2026 failure-detection paper we should also be comparing against?

## What I searched for

Strict criteria: a peer-reviewed or arXiv'd paper from 2023 onward proposing a concrete, implementable failure-detection algorithm with the same interface as our five detectors (heartbeat stream in, suspicion decision out).

Search terms covered: accrual extensions, ML/LSTM-based detectors, transformer/foundation-model anomaly work, heavy-tailed heartbeat modeling, SWIM/gossip variants.

## What I found

### Primary 2024 anchor — a survey, not an algorithm

> Bhavana Chaurasia, Anshul Verma, Pradeepika Verma. **"An in-depth and insightful exploration of failure detection in distributed systems."** *Computer Networks*, Volume 247, June 2024. DOI: 10.1016/j.comnet.2024.110432.

Systematic literature review. Taxonomy (heartbeat, accrual, gossip/polling) maps one-to-one onto our five implementations — we are not missing a category. Identifies AI/ML integration as the main open future direction.

**Use:** framing reference for the manuscript. Cite as the state-of-the-practice survey, position our work as the controlled empirical comparison of its taxonomy.

### Closest concrete algorithm — 2022, not 2023+

> Xiaonan Li, Olivier Marin. **"Towards Implementing ML-Based Failure Detectors."** EDCC 2022 Student Forum. arXiv:2210.00134.

LSTM predictor, asymmetric loss function penalizing under-prediction, dynamic safety margin from recent prediction errors. Claims ~95% accuracy on real traces. Four pages (student forum), so methodological detail is thin.

**Technically 2022**, but it is the most recent concrete algorithmic proposal in this sub-field that is directly comparable to our five detectors. Nothing newer passes the "could we implement it in Rust and drop it into our `FailureDetector` trait?" test.

### What the 2023–2025 literature actually contains

- **Surveys** (Chaurasia et al. 2024; Kirti et al. 2024 in *Concurrency and Computation*). Useful framing, not benchmarkable.
- **AIOps / LLM-based failure management** (several 2024–2025 arXiv surveys on LLMs for cloud-ops incident detection). Operates at log/metric-stream scale, not heartbeat scale. Not a drop-in replacement for φ-accrual.
- **Production reliability studies** at hyperscale (Meta LLaMA 3 training, ByteDance, Google TPU). Motivating context; no specific algorithm to compare against.

## Decision

**No implementation today.** The honest finding is that there is no 2023+ algorithm paper to benchmark against in this sub-field yet — the gap is itself a citable observation, and the 2024 survey names it (AI/ML integration) as the primary open direction.

The right next implementation target is the Li & Marin 2022 LSTM detector. Not a weekend item (Rust ML stack choice, training regime, determinism for a stochastic-init model, separating training from simulation time in the event loop). Promoted from "maybe" to **P1 in NEXT.md** after the jitter-distribution work.

## Files changed

- `docs/overview.md` — new §11 ("Positioning against recent literature (2023+)") covering Chaurasia 2024, Li & Marin 2022, and the explicit negative finding on 2023+ algorithms. Added a §8 Q&A entry for mentor conversation.
- `docs/logs/2026-04-16_recent-literature-check.md` — this file.

No code changes. Nothing to commit (per session-durable instruction to keep local).

## Caveats

- Search was web-based, not a proper literature database sweep. There could be a 2023+ venue-specific paper that did not surface — worth asking the mentor if they know of one before locking the manuscript's related-work section.
- Li & Marin 2022 being a 4-page student forum paper means the claimed 95% accuracy result is not fully characterized (window size, training regime, baseline comparisons are not in the paper body). Implementing it will involve design choices the paper does not pin down.
