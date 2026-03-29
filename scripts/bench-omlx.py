#!/usr/bin/env python3
"""
oMLX Phase Classifier Benchmark — measures real throughput + latency.

Tests:
  1. Single-call latency (cold + warm)
  2. Sequential throughput (calls/sec)
  3. Concurrent throughput (2/4 parallel slots)
  4. Payload size impact (small/medium/large context)
  5. Model load detection (probe until ready)

Usage:
  python3 scripts/bench-omlx.py [--port 10710] [--rounds 10]
"""

import argparse
import json
import time
import sys
import statistics
from concurrent.futures import ThreadPoolExecutor, as_completed
from urllib.request import Request, urlopen
from urllib.error import URLError

SYSTEM_PROMPT = """Classify this AI coding session. Output ONLY a JSON object, nothing else.

{"phase": "...", "scope": "..."}

phase — exactly one of: thinking, planning, building, testing, reviewing, shipping
scope — 3-8 word description of the task

Use the Signals section (files, commands, tools) alongside the conversation to determine the phase."""

PAYLOADS = {
    "tiny": "User: hi\nAssistant: hello",
    "small": """## Session Goal
Fix the auth bug in login.ts

## Recent Activity
User: fix the auth bug
Assistant: Reading file [tools: Read]
Assistant: Found the issue, updating [tools: Edit]""",
    "medium": """## Session Goal
Refactor auth middleware to use JWT tokens instead of session cookies

## Signals
Files referenced: src/middleware/auth.ts, tests/auth.test.ts
Files edited: src/middleware/auth.ts
Commands: bun test auth; cargo check
Tools: Edit:12 Read:8 Bash:5 Write:2

## Recent Activity
User: refactor the auth middleware to use JWT
Assistant: I'll start by reading the current auth implementation [tools: Read]
User: also update the tests
Assistant: Reading the test file [tools: Read]
Assistant: Found 3 tests that need updating. Starting with the login test [tools: Edit]
User: run the tests
Assistant: Running the test suite [tools: Bash]
Assistant: 2 passed, 1 failed - the refresh token test expects cookies [tools: Edit]
Assistant: Fixed the refresh test, running again [tools: Bash]
Assistant: All 3 tests passing now""",
    "large": """## Session Goal
Implement a complete OAuth2 integration with Google, GitHub, and Microsoft providers. Need PKCE flow, token refresh, and session management with Redis backing.

## Signals
Files referenced: src/auth/oauth.ts, src/auth/providers/google.ts, src/auth/providers/github.ts, src/auth/session.ts, src/redis/client.ts
Files edited: src/auth/oauth.ts, src/auth/providers/google.ts, src/auth/providers/github.ts, src/auth/session.ts
Commands: bun test auth; bun test oauth; redis-cli ping; curl localhost:3000/auth/google/callback
Tools: Edit:24 Read:18 Bash:12 Write:6 Grep:4

## Recent Activity
User: implement oauth2 with google github and microsoft
Assistant: I'll start by setting up the OAuth2 base class with PKCE support [tools: Read, Read, Edit]
User: use the standard openid connect discovery
Assistant: Good call, implementing OIDC discovery endpoint resolution [tools: Edit, Edit]
Assistant: Base OAuth class done with PKCE. Now adding Google provider [tools: Write]
User: make sure refresh tokens work
Assistant: Adding token refresh logic with Redis session storage [tools: Read, Edit, Edit]
Assistant: Google provider complete. Moving to GitHub [tools: Write, Edit]
User: github doesn't support OIDC
Assistant: Right, GitHub uses their own OAuth flow. Implementing custom handler [tools: Edit]
Assistant: GitHub provider done. Now Microsoft with Azure AD [tools: Write]
User: test the full flow
Assistant: Running the OAuth integration tests [tools: Bash]
Assistant: Google and GitHub passing. Microsoft has a scope issue [tools: Edit]
Assistant: Fixed Microsoft scope format. All providers passing [tools: Bash]
User: check the redis session expiry
Assistant: Verifying Redis TTL on sessions [tools: Bash, Bash]
Assistant: Sessions expire correctly at 7 days. Refresh tokens at 30 days [tools: Read]""",
}


def make_request(base_url: str, model: str, payload: str, max_tokens: int = 80) -> dict:
    """Make a single classify request, return timing + result."""
    body = json.dumps({
        "model": model,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": payload},
        ],
        "temperature": 0.15,
        "max_tokens": max_tokens,
        "chat_template_kwargs": {"enable_thinking": False},
    }).encode()

    req = Request(
        f"{base_url}/v1/chat/completions",
        data=body,
        headers={"Content-Type": "application/json"},
    )

    t0 = time.monotonic()
    try:
        with urlopen(req, timeout=10) as resp:
            result = json.loads(resp.read())
            latency_ms = (time.monotonic() - t0) * 1000
            content = result["choices"][0]["message"]["content"]
            tokens_in = result.get("usage", {}).get("prompt_tokens", 0)
            tokens_out = result.get("usage", {}).get("completion_tokens", 0)
            return {
                "ok": True,
                "latency_ms": latency_ms,
                "content": content,
                "tokens_in": tokens_in,
                "tokens_out": tokens_out,
            }
    except Exception as e:
        latency_ms = (time.monotonic() - t0) * 1000
        return {"ok": False, "latency_ms": latency_ms, "error": str(e)[:100]}


def check_server(base_url: str, model_substr: str) -> str | None:
    """Check if oMLX is up and model is loaded. Returns model ID or None."""
    try:
        with urlopen(f"{base_url}/v1/models", timeout=3) as resp:
            data = json.loads(resp.read())
            for m in data.get("data", []):
                if model_substr in m["id"]:
                    return m["id"]
    except Exception:
        pass
    return None


def fmt_lat(values: list[float]) -> str:
    if not values:
        return "no data"
    return (
        f"min={min(values):.0f}ms  "
        f"p50={statistics.median(values):.0f}ms  "
        f"avg={statistics.mean(values):.0f}ms  "
        f"p90={sorted(values)[int(len(values)*0.9)]:.0f}ms  "
        f"max={max(values):.0f}ms"
    )


def run_benchmark(base_url: str, model: str, rounds: int):
    print(f"\n{'='*60}")
    print(f"oMLX Benchmark — {model} @ {base_url}")
    print(f"{'='*60}\n")

    # ── Test 1: Probe until inference works ──
    print("Test 1: Model readiness probe")
    probe_start = time.monotonic()
    for i in range(30):
        r = make_request(base_url, model, "hi", max_tokens=1)
        if r["ok"]:
            probe_ms = (time.monotonic() - probe_start) * 1000
            print(f"  Ready after {i+1} probes ({probe_ms:.0f}ms total)")
            print(f"  First inference: {r['latency_ms']:.0f}ms")
            break
        time.sleep(1)
    else:
        print("  FAILED: model not ready after 30s")
        sys.exit(1)

    # ── Test 2: Cold vs warm latency ──
    print(f"\nTest 2: Single-call latency ({rounds} rounds)")
    lats = {}
    for name, payload in PAYLOADS.items():
        results = []
        for _ in range(rounds):
            r = make_request(base_url, model, payload)
            if r["ok"]:
                results.append(r["latency_ms"])
        lats[name] = results
        tok_in = r.get("tokens_in", "?") if r["ok"] else "?"
        print(f"  {name:>8} ({tok_in:>4} tok): {fmt_lat(results)}")

    # ── Test 3: Sequential throughput ──
    print(f"\nTest 3: Sequential throughput ({rounds} calls, medium payload)")
    t0 = time.monotonic()
    seq_lats = []
    for _ in range(rounds):
        r = make_request(base_url, model, PAYLOADS["medium"])
        if r["ok"]:
            seq_lats.append(r["latency_ms"])
    seq_dur = time.monotonic() - t0
    seq_qps = len(seq_lats) / seq_dur if seq_dur > 0 else 0
    print(f"  {fmt_lat(seq_lats)}")
    print(f"  Throughput: {seq_qps:.2f} calls/sec (1 slot)")

    # ── Test 4: Concurrent throughput ──
    for slots in [2, 4]:
        print(f"\nTest 4: Concurrent throughput ({slots} slots, {rounds} calls each)")
        total_calls = slots * rounds
        t0 = time.monotonic()
        con_lats = []
        with ThreadPoolExecutor(max_workers=slots) as pool:
            futures = [
                pool.submit(make_request, base_url, model, PAYLOADS["medium"])
                for _ in range(total_calls)
            ]
            for f in as_completed(futures):
                r = f.result()
                if r["ok"]:
                    con_lats.append(r["latency_ms"])
        con_dur = time.monotonic() - t0
        con_qps = len(con_lats) / con_dur if con_dur > 0 else 0
        print(f"  {fmt_lat(con_lats)}")
        print(f"  Throughput: {con_qps:.2f} calls/sec ({slots} slots)")
        print(f"  Speedup vs sequential: {con_qps/seq_qps:.1f}x" if seq_qps > 0 else "")

    # ── Summary ──
    print(f"\n{'='*60}")
    print("SUMMARY")
    print(f"{'='*60}")
    all_ok = sum(len(v) for v in lats.values()) + len(seq_lats) + len(con_lats)
    print(f"  Total successful calls: {all_ok}")
    all_lats = [l for v in lats.values() for l in v] + seq_lats + con_lats
    if all_lats:
        print(f"  Overall: {fmt_lat(all_lats)}")
        under_500 = sum(1 for l in all_lats if l < 500)
        under_1000 = sum(1 for l in all_lats if l < 1000)
        print(f"  <500ms: {under_500}/{len(all_lats)} ({under_500/len(all_lats)*100:.0f}%)")
        print(f"  <1000ms: {under_1000}/{len(all_lats)} ({under_1000/len(all_lats)*100:.0f}%)")
    print()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="oMLX Phase Classifier Benchmark")
    parser.add_argument("--port", type=int, default=10710)
    parser.add_argument("--rounds", type=int, default=10)
    args = parser.parse_args()

    base_url = f"http://localhost:{args.port}"
    model = check_server(base_url, "Qwen3.5")
    if not model:
        print(f"oMLX not reachable at {base_url} or model not loaded")
        sys.exit(1)

    run_benchmark(base_url, model, args.rounds)
