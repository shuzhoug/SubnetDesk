# SubnetDesk Enterprise Implementation Roadmap

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the first enterprise release as three independently testable subsystems: Laravel control plane, Rust Device Gateway, and SubnetDesk enterprise client with low-latency remote-control mode.

**Architecture:** The Laravel application owns durable multi-tenant business state and authorization. The Rust Device Gateway owns authenticated device presence and heartbeats backed by Redis. The existing SubnetDesk direct remote-control data path remains peer-to-peer; the client adds enterprise enrollment, cloud device discovery, authorization-before-connect, and performance presets.

**Tech Stack:** Laravel 12+, PHP 8.3+, MySQL 8, Redis 7, Rust stable, Tokio, Axum or Actix Web, Flutter/Dart, existing SubnetDesk/RustDesk remote core.

**Spec:** `docs/superpowers/specs/2026-08-21-enterprise-management-design.md`

## Global Constraints

- One hosted platform serves multiple tenants.
- Business data is tenant-scoped by `tenant_id`.
- A tenant may manage roughly 1,000 to 10,000 devices.
- Device presence and heartbeats do not generate synchronous MySQL writes.
- Remote screen/audio/input/file-transfer payloads never flow through Laravel or the Device Gateway.
- Enrollment codes are bootstrap-only and are not persisted after successful enrollment.
- Managed remote connections require a successful authorization check before invoking the existing SubnetDesk connection path.
- Existing LAN discovery and direct IP/hostname connection must continue to work.
- TDD is mandatory for production feature code.

---

## Delivery Order

### Plan A — Laravel Control Plane

File: `docs/superpowers/plans/2026-08-21-laravel-control-plane.md`

Deliverables:

1. multi-tenant schema and tenant-scoped models;
2. authentication, roles, permissions, departments;
3. enrollment token and device enrollment API;
4. device/group/policy APIs;
5. Redis-backed presence projection;
6. remote authorization and resource-scope checks;
7. audit and remote-session lifecycle APIs.

Acceptance gate before Plan B integration: an API test suite can create two tenants and prove cross-tenant device reads, enrollment, and remote authorization are rejected.

### Plan B — Rust Device Gateway

File: `docs/superpowers/plans/2026-08-21-device-gateway.md`

Deliverables:

1. secure WebSocket device endpoint;
2. credential authentication;
3. heartbeat processing;
4. Redis presence writes and TTL handling;
5. policy-change notifications;
6. horizontal-scaling ownership metadata;
7. representative 10,000-device heartbeat load test.

Acceptance gate before Plan C integration: a synthetic client can authenticate, maintain heartbeats, become visible in Redis, disconnect, and transition offline without MySQL writes.

### Plan C — SubnetDesk Enterprise Client

File: `docs/superpowers/plans/2026-08-21-subnetdesk-enterprise-client.md`

Deliverables:

1. enterprise configuration and secure device identity storage;
2. employee login and enrollment-code workflows;
3. Gateway connection and heartbeat;
4. cloud-managed device list in Flutter;
5. authorization-before-connect integration;
6. session audit events;
7. preconfigured installer bootstrap;
8. low-latency / balanced / high-quality performance presets;
9. regression tests proving LAN-only direct use still works.

## Integration Milestones

### Milestone 1: Enrollment vertical slice

- [ ] Laravel creates an enrollment token.
- [ ] A desktop SubnetDesk build submits token + device metadata.
- [ ] Laravel creates a device and returns a permanent device credential.
- [ ] Client stores the permanent credential securely and discards the enrollment token.

### Milestone 2: Presence vertical slice

- [ ] Client connects to Gateway with permanent credential.
- [ ] Gateway authenticates it.
- [ ] Gateway writes `device:presence:{uuid}` and tenant online-set membership to Redis.
- [ ] Laravel device list reports the device online.
- [ ] Killing the client causes status to become offline after the configured TTL.

### Milestone 3: Managed remote-control vertical slice

- [ ] User selects a cloud-managed device in Flutter.
- [ ] Client calls `POST /api/v1/remote-authorizations`.
- [ ] Laravel enforces tenant + permission + resource scope.
- [ ] If allowed, client invokes the existing direct SubnetDesk connection path.
- [ ] Session start/end events are persisted.
- [ ] Remote media bypasses Laravel and Gateway.

### Milestone 4: Enterprise deployment vertical slice

- [ ] Admin creates a deployment enrollment token.
- [ ] Windows installer/config is produced with control-plane URL + one-time enrollment token.
- [ ] First launch auto-enrolls.
- [ ] Bootstrap token is removed locally after success.
- [ ] Device receives its department/group defaults and policy.

### Milestone 5: Performance acceptance

- [ ] Balanced, Low Latency, and High Quality presets are exposed in UI.
- [ ] Low Latency prefers hardware codec when available.
- [ ] Low Latency may reduce resolution/quality/frame-rate before allowing frame queue growth.
- [ ] Input traffic remains responsive during bandwidth pressure and concurrent file transfer.
- [ ] A load test simulates 10,000 connected Gateway clients at a 30-second heartbeat interval.

## Repository Layout

Keep the current `shuzhoug/SubnetDesk` repository as the client repository.

Create separate repositories before implementation for:

```text
SubnetDesk-ControlPlane   # Laravel
SubnetDesk-Gateway        # Rust/Tokio Gateway
SubnetDesk                # existing client
```

If repository creation is handled outside the implementation session, initialize each with its own CI, README, `.env.example`, and deployment documentation before feature work begins.

## Completion Definition

The first enterprise release is ready for pilot deployment only when all three subsystem plans pass their own automated tests plus an end-to-end test covering enrollment, online presence, authorization, direct connection initiation, and session audit.