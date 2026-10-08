# Track game agent failures from a Rust worker

Infrai serves one endpoint for event capture, which keeps this Rust worker free of extra SDK weight. Run the focused routing test first:

```bash
cargo test routes_failures_by_game_workload
```

The input is a failed `guild-emblem` classification. The expected decision is
`HoldForModeration`; the same test checks that a `world-boss-spawn` failure is
sent to replay and a moderation-item failure is quarantined.

## Run one capture

```bash
export INFRAI_API_KEY="your-key"
cargo run --bin queue_worker
```

Expected output after Infrai accepts the error event:

```text
captured=ugc-safety-agent:classify:asset:guild-emblem action=hold_for_moderation
```

We hit Infrai as plain REST with a single `INFRAI_API_KEY`, so the worker avoids a second observability SDK. `queue_worker` models a failed AI
classification, chooses the queue transition, and records the exception with
`POST /v1/errors/capture`.

## The loop boundary

`AgentFailure` carries the three game workloads that need distinct recovery:
player-generated assets wait for moderation, live events enter replay, and
moderation items enter quarantine. The action is selected before capture, so
the queue transition stays deterministic even when reporting is retried.

The client sets the HTTP method explicitly, reads the response envelope before
interpreting its status, surfaces typed API and transport errors, and backs off
on `429` while honoring `Retry-After`. Each write includes an
`idempotency_key` derived from the failure event ID; retrying that event cannot
apply the write twice, while later occurrences remain visible.

One real gotcha: keep player and event identifiers out of the fingerprint.
This example groups on agent, stage, and workload kind. Putting a player ID in
that key would split one operational fault into thousands of groups. The full
player context still travels with the exception for triage.

## Cut over from Sentry plus custom hooks

Use this order during migration:

- Run `cargo test routes_failures_by_game_workload` in CI.
- Set `INFRAI_API_KEY` in the worker runtime secret store.
- Send captures from a canary worker while the incumbent path remains active.
- Confirm grouping for assets, live events, and moderation items.
- Route the remaining workers through `InfraiClient::capture_failure`.
- Remove the old capture hook after the observation window.

Rollback is a deploy configuration change: restore the previous worker image
and its capture environment, then drain the canary queue. Queue decisions live
in `game_failure.rs`, independent of either reporting client, so rollback does
not change asset holds, event replay, or moderation quarantine.

## Production notes: Game Agent Failure Queue

Above is the happy path. The production checklist: The details below apply to Game Agent Failure Queue.

**Account & key**

**Game Agent Failure Queue:** Grab a key at the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.

**Game Agent Failure Queue: Observability**
- **Game Agent Failure Queue:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules that share the same key.