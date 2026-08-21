# Device Gateway Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a horizontally scalable Rust service that authenticates enrolled SubnetDesk devices, maintains persistent WebSocket connections, and stores live presence in Redis without carrying remote desktop media.

**Architecture:** A Tokio-based Gateway terminates TLS behind a reverse proxy or directly, authenticates per-device credentials, keeps one logical connection owner per online device, refreshes Redis presence TTLs on heartbeats, and emits lightweight policy-change/control notifications. Durable business data remains in Laravel/MySQL.

**Tech Stack:** Rust stable, Tokio, Axum or Actix Web, tokio-tungstenite/axum WebSocket, Redis 7, serde/serde_json, tracing, criterion/k6 or a Rust load harness.

**Spec:** `docs/superpowers/specs/2026-08-21-enterprise-management-design.md`

## Global Constraints

- No screen/audio/input/file payload proxying.
- Normal heartbeat interval starts at 30 seconds; presence TTL starts at 90 seconds.
- Credential secrets never appear in URL query strings or logs.
- Redis stores current presence and gateway ownership.
- Gateway failure must not force existing peer-to-peer sessions to terminate.
- TDD is mandatory.

---

### Task 1: Bootstrap Gateway Service

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/config.rs`
- Create: `src/http.rs`
- Create: `tests/health.rs`

**Interfaces:**
- Produces: `GET /healthz` and `GET /readyz`.

- [ ] Write failing integration test for `GET /healthz` returning 200.
- [ ] Run `cargo test` and verify RED.
- [ ] Implement minimal Tokio HTTP server and typed configuration.
- [ ] Verify GREEN.
- [ ] Commit `feat: bootstrap device gateway`.

### Task 2: Versioned Device Protocol Types

**Files:**
- Create: `src/protocol/mod.rs`
- Create: `src/protocol/v1.rs`
- Test: `tests/protocol_v1.rs`

**Interfaces:**
- Produces enums/structs for `hello`, `heartbeat`, `policy_changed`, `error`, and server acknowledgement messages.

- [ ] Write serde round-trip tests for exact v1 JSON examples from the design spec.
- [ ] Reject unknown protocol versions with an explicit protocol error.
- [ ] Implement types with strict field validation where useful.
- [ ] Commit `feat: define gateway protocol v1`.

### Task 3: Device Credential Authentication

**Files:**
- Create: `src/auth/mod.rs`
- Create: `src/auth/device_credentials.rs`
- Create: `src/control_plane/client.rs`
- Test: `tests/device_auth.rs`

**Interfaces:**
- Produces: `DeviceAuthenticator::authenticate(credential_id, proof) -> AuthenticatedDevice`.

- [ ] Write failing tests for valid, revoked, unknown, and wrong credentials.
- [ ] Implement an internal control-plane lookup interface with cacheable credential metadata; do not log secrets.
- [ ] Use constant-time verification for opaque-secret mode.
- [ ] Verify all authentication tests pass.
- [ ] Commit `feat: authenticate enrolled devices`.

### Task 4: Authenticated WebSocket Session

**Files:**
- Create: `src/ws/mod.rs`
- Create: `src/ws/session.rs`
- Modify: `src/http.rs`
- Test: `tests/websocket_session.rs`

**Interfaces:**
- Produces endpoint: `GET /device/v1` upgrade to WebSocket.
- Produces session state containing authenticated `device_id`, `tenant_id`, `connection_id`, `gateway_id`.

- [ ] Test unauthenticated clients cannot enter active session state.
- [ ] Test one authenticated client receives hello acknowledgement.
- [ ] Implement bounded inbound/outbound queues to prevent memory growth from slow peers.
- [ ] Commit `feat: add authenticated device websocket`.

### Task 5: Redis Presence Repository

**Files:**
- Create: `src/presence/mod.rs`
- Create: `src/presence/redis.rs`
- Test: `tests/presence_redis.rs`

**Interfaces:**
- Produces: `PresenceRepository::online`, `heartbeat`, `offline`.
- Writes `device:presence:{uuid}` and `tenant:{tenant_id}:online_devices`.

- [ ] Write tests using an isolated Redis database/container.
- [ ] Verify `online` writes expected fields and 90-second TTL.
- [ ] Verify `heartbeat` refreshes TTL and endpoint/version metadata.
- [ ] Verify `offline` removes tenant-set membership only if connection ownership matches.
- [ ] Commit `feat: store realtime device presence`.

### Task 6: Heartbeat Processing

**Files:**
- Create: `src/ws/heartbeat.rs`
- Modify: `src/ws/session.rs`
- Test: `tests/heartbeat.rs`

**Interfaces:**
- Consumes v1 heartbeat messages.
- Produces presence refresh and heartbeat acknowledgement.

- [ ] Test valid heartbeat updates Redis without calling MySQL/control-plane writes.
- [ ] Test malformed heartbeat closes or rejects according to protocol policy.
- [ ] Test heartbeat from a mismatched device identity is rejected.
- [ ] Implement monotonic session timing for timeout decisions.
- [ ] Commit `feat: process device heartbeats`.

### Task 7: Connection Ownership and Reconnect Safety

**Files:**
- Create: `src/presence/ownership.rs`
- Modify: `src/ws/session.rs`
- Test: `tests/connection_ownership.rs`

**Interfaces:**
- Produces atomic ownership compare-and-set semantics using `gateway_id + connection_id`.

- [ ] Test old socket disconnect cannot mark a newly reconnected socket offline.
- [ ] Implement ownership token in Redis presence hash.
- [ ] Ensure duplicate connections follow a deterministic policy: newest authenticated connection wins.
- [ ] Commit `fix: make device reconnect presence safe`.

### Task 8: Policy Change Notification

**Files:**
- Create: `src/control_plane/events.rs`
- Create: `src/ws/outbound.rs`
- Test: `tests/policy_notification.rs`

**Interfaces:**
- Consumes internal message `{device_id, policy_version}`.
- Produces WebSocket `policy_changed` notification.

- [ ] Test notification reaches only the owning connection.
- [ ] Test offline device notification is safely dropped/queued by control-plane policy, not persisted indefinitely in Gateway memory.
- [ ] Implement lightweight internal authenticated endpoint or Redis pub/sub channel.
- [ ] Commit `feat: push policy change notifications`.

### Task 9: Resilience and Backoff Semantics

**Files:**
- Modify: `src/presence/redis.rs`
- Modify: `src/ws/session.rs`
- Create: `src/metrics.rs`
- Test: `tests/redis_outage.rs`

**Interfaces:**
- Exposes metrics: active connections, auth failures, heartbeat rate, Redis errors, reconnects.

- [ ] Test temporary Redis failure does not immediately close healthy WebSocket sessions.
- [ ] Implement bounded retries/backoff for presence updates.
- [ ] Ensure readiness becomes degraded when Redis is unavailable while liveness remains healthy.
- [ ] Commit `feat: harden gateway dependencies`.

### Task 10: 10,000-Device Load Harness

**Files:**
- Create: `loadtest/Cargo.toml`
- Create: `loadtest/src/main.rs`
- Create: `docs/load-testing.md`

**Interfaces:**
- Simulates configurable device count, heartbeat interval, reconnect percentage.

- [ ] Implement synthetic authenticated clients using test credentials issued by a test control-plane stub.
- [ ] Run 1,000-device smoke test first.
- [ ] Run representative 10,000-device test with 30-second heartbeats.
- [ ] Capture connection count, heartbeat throughput, CPU, memory, Redis ops/sec, and p95 heartbeat processing latency.
- [ ] Verify no synchronous MySQL dependency exists in heartbeat path.
- [ ] Commit `test: add gateway scale harness`.

### Task 11: CI and Containerization

**Files:**
- Create: `Dockerfile`
- Create: `.github/workflows/test.yml`
- Create: `.env.example`
- Create: `README.md`

- [ ] Configure CI for `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`.
- [ ] Document required environment variables without secrets.
- [ ] Build container and run health/readiness checks locally.
- [ ] Commit `build: add gateway CI and container`.

## Plan Acceptance

The Gateway plan is complete when an enrolled synthetic device can authenticate, maintain a persistent connection, refresh Redis presence every 30 seconds, reconnect safely, receive policy notifications, and a 10,000-client load run completes without remote media passing through the service or heartbeat-triggered MySQL writes.