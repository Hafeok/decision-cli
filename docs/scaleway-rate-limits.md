# Scaleway Generative APIs — Serverless rate limits

Reference for which Scaleway quota we hit when `dec drive ship` fails with a
LiteLLM `RateLimitError`/`OpenAIException` 429. Sourced from
<https://www.scaleway.com/en/docs/organizations-and-projects/additional-content/organization-quotas/#generative-apis---serverless>
(reviewed October 29, 2025). Per-Organization, shared across all Projects.

> The catch-all hint: hitting 429 with `INSUFFICIENT QUOTA` and the phrase
> `tokens per minute` means **TPM**, not RPM. The bundle (spec body + linked
> ADRs + linked TCs + system prompt + tool defs) exceeded the per-minute
> ceiling on a single turn.

## Three dimensions, two tiers, one common error

Every request is metered against three limits simultaneously. Hitting any of
them returns HTTP **429** with body `{"status":429,"error":"INSUFFICIENT QUOTA","message":"..."}`.

- **TPM** — Tokens per minute (input + output combined).
- **RPM** — Requests per minute (HTTP request count).
- **Concurrent requests** — Simultaneous in-flight requests.

Tiers:

- **Base** — Payment method validated.
- **Identity** — Payment method + identity validated (auto-upgrade after
  https://console.scaleway.com identity verification).

`x-ratelimit-{limit,remaining,reset}-{requests,tokens}` response headers
report live usage on every response, not just 429s.

## TPM (tokens per minute)

| Model | Base | Identity |
|---|---:|---:|
| gemma-3-27b-it | 200k | 400k |
| llama-3.3-70b-instruct | 200k | 400k |
| llama-3.1-70b-instruct | 200k | 400k |
| llama-3.1-8b-instruct | 200k | 400k |
| deepseek-r1-distill-llama-70b | 200k | 400k |
| mistral-small-3.1-24b-instruct-2503 | 200k | 400k |
| **mistral-small-3.2-24b-instruct-2506** | 200k | **1 000k** |
| mistral-nemo-instruct-2407 | 200k | 400k |
| voxtral-small-24b-2507 | 200k | 400k |
| pixtral-12b-2409 | 200k | 400k |
| **qwen3.5-397b-a17b** | 200k | **1 000k** |
| **qwen3-235b-a22b-instruct-2507** | 200k | **1 000k** |
| **qwen3-embedding-8b** | 200k | **1 000k** |
| **`qwen3-coder-30b-a3b-instruct`** ← decision-cli's code-writer | **200k** | **400k** |
| qwen2.5-coder-32b-instruct | 200k | 400k |
| **gpt-oss-120b** | 200k | **1 000k** |
| holo2-30b-a3b | 200k | 400k |
| bge-multilingual-gemma2 | 200k | 400k |

## RPM (requests per minute)

| Model | Base | Identity |
|---|---:|---:|
| All listed models (including `qwen3-coder-30b-a3b-instruct`) | **300** | **600** |
| whisper-large-v3 | 300 | 600 |

## Concurrent requests

| Bucket | Base | Identity |
|---|---:|---:|
| All models | **50** | **50** |
| Concurrent batches | 20 | 100 |

## Audio seconds per minute

| Model | Base | Identity |
|---|---:|---:|
| voxtral-small-24b-2507 | 1800 | 3600 |
| whisper-large-v3 | 1800 | 3600 |

## What this means for decision-cli right now

The code-writer capability binding points at **`qwen3-coder-30b-a3b-instruct`**
which sits in the **400k TPM (identity-tier)** bucket. That's the
**second-lowest** family — most other models we could swap to don't go higher
unless we also accept a different model family.

Worth knowing for diagnosis:

- **A single FT-139-class bundle is ~50–100k tokens** (spec body + 2 ADRs + 5
  TC bodies + system prompt + tool defs). One turn nominally fits inside the
  400k budget; iteration loops with growing context push past it quickly.
- **The `max_turns: 64` we set is a turn cap, not a token cap.** A long-running
  loop can blow past TPM long before turn 64.
- **Witnessed pattern.** FT-139 + cascade-3 retries all 429'd on TPM, not RPM:
  the error message specifically says `"tokens per minute"`. RPM (300/600) is
  nowhere near saturated for our usage shape.

## Bypass paths (per Scaleway docs)

In order of operator effort:

1. **Verify identity** — auto-upgrades from 200k → 400k–1000k TPM depending on
   model. Free, one-time.
2. **Batches API** — no rate limit, billed -50% vs standard. Suitable for
   non-real-time work (our drive ship loops are real-time interactive — not a
   fit).
3. **Switch to a 1M-tier model** for the code-writer capability binding —
   `qwen3.5-397b-a17b`, `qwen3-235b-a22b-instruct-2507`,
   `mistral-small-3.2-24b-instruct-2506`, `gpt-oss-120b`. Tradeoff: bigger
   models are more expensive per token and may be slower per turn.
4. **Generative APIs — Dedicated Deployment** — no rate limit, you pay for
   provisioned capacity. Out of scope for now.
5. **Contact Sales** for volume commitment + custom quota.

## Triaging a future 429

When `dec drive ship` returns:

```
RateLimitError: OpenAIException - Error code: 429
{"status":429,"error":"INSUFFICIENT QUOTA","message":"..."}
```

Grep the message text:

- `tokens per minute` → TPM cap. Either wait ~60s for the sliding window to
  clear, swap to a 1M-tier model, or upgrade to a Batches API path.
- `requests per minute` → RPM cap. Very unlikely with our usage shape; if it
  appears, something is spamming.
- `concurrent` → too many in-flight. Lower parallelism.
- No specific dimension named → check response headers
  `x-ratelimit-remaining-tokens` and `x-ratelimit-remaining-requests` from the
  preceding successful call.
