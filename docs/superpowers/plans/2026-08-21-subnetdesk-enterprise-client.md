# SubnetDesk Enterprise Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add enterprise enrollment, Gateway presence, cloud-managed device discovery, managed authorization-before-connect, session audit, and low-latency performance presets to the existing SubnetDesk client without breaking LAN-only direct remote control.

**Architecture:** New enterprise code is isolated under focused Rust and Flutter modules. The existing remote-control engine remains authoritative for capture, encoding, input, audio, clipboard, file transfer, and direct connection. Enterprise APIs decide identity, policy, and whether a managed connection may start.

**Tech Stack:** Existing SubnetDesk Rust workspace, Tokio, Flutter/Dart, existing FFI patterns, platform credential stores, existing hardware codec support.

**Spec:** `docs/superpowers/specs/2026-08-21-enterprise-management-design.md`

## Global Constraints

- Existing LAN discovery and manual IP/hostname direct connection remain functional.
- Cloud management must not route remote media through Laravel or Gateway.
- Enrollment tokens are removed after successful enrollment.
- Device credentials are persisted in the operating system's secure credential facility where practical.
- Managed remote sessions perform authorization before invoking the existing direct connection flow.
- Low-latency mode prioritizes interaction latency over image fidelity under constrained bandwidth.
- TDD is mandatory for new feature behavior.

---

### Task 1: Locate Stable Integration Seams and Add Enterprise Module Shell

**Files:**
- Create: `src/enterprise/mod.rs`
- Create: `flutter/lib/enterprise/enterprise.dart`
- Modify only the minimum existing module declarations required to expose the new modules.
- Test: Rust unit/module compilation tests and Flutter analyzer test target.

**Interfaces:**
- Produces an enterprise namespace without changing existing remote behavior.

- [ ] Add a failing/compile-time test referencing the new enterprise configuration type.
- [ ] Create the minimal Rust and Flutter module shells.
- [ ] Run Rust tests and `flutter analyze`.
- [ ] Commit `feat: add enterprise client module boundaries`.

### Task 2: Enterprise Configuration Model

**Files:**
- Create: `src/enterprise/config.rs`
- Create: `flutter/lib/enterprise/models/enterprise_config.dart`
- Add FFI bridge using the repository's existing Flutter/Rust integration pattern.
- Test: Rust config serialization tests; Flutter model tests.

**Interfaces:**
- Fields: `control_plane_url`, `tenant_id`, `device_id`, `credential_id`, `gateway_url`, `policy_version`.

- [ ] Write round-trip tests proving non-secret config is persisted and loaded exactly.
- [ ] Implement config separation so credential secret/private key is not serialized into the same plaintext config.
- [ ] Commit `feat: add enterprise client configuration`.

### Task 3: Secure Device Credential Storage

**Files:**
- Create: `src/enterprise/secure_store.rs`
- Create platform-specific implementations following existing `src/platform/` conventions where necessary.
- Test: `src/enterprise/secure_store.rs` unit tests plus platform adapter tests where CI permits.

**Interfaces:**
- Produces: `store_device_secret`, `load_device_secret`, `delete_device_secret` keyed by credential ID.

- [ ] Write failing tests against an injectable secure-store trait and in-memory test implementation.
- [ ] Implement Windows DPAPI/Credential Manager-backed storage path.
- [ ] Implement macOS Keychain-backed storage path.
- [ ] Implement Linux Secret Service path with documented secure fallback behavior.
- [ ] Ensure debug formatting never prints secrets.
- [ ] Commit `feat: persist device credentials securely`.

### Task 4: Enterprise HTTPS API Client

**Files:**
- Create: `src/enterprise/api.rs`
- Create: `flutter/lib/enterprise/api/enterprise_api.dart` only for UI-facing methods that are not better kept in Rust.
- Test: Rust HTTP client tests against a local mock server.

**Interfaces:**
- Produces client methods for login, enrollment, device list, policy fetch, remote authorization, session started, session ended.

- [ ] Write tests for success, timeout, TLS/HTTP error mapping, and secret redaction.
- [ ] Implement typed request/response structures matching `/api/v1` design.
- [ ] Add bounded request timeouts so cloud failure does not hang the UI.
- [ ] Commit `feat: add enterprise control plane client`.

### Task 5: Enrollment by Code

**Files:**
- Create: `src/enterprise/enrollment.rs`
- Create: `flutter/lib/enterprise/enrollment/enrollment_page.dart`
- Create: `flutter/lib/enterprise/enrollment/enrollment_controller.dart`
- Test: Rust enrollment tests; Flutter controller/widget tests.

**Interfaces:**
- Consumes plaintext one-time enrollment token.
- Produces permanent config + secure device credential.

- [ ] Test successful enrollment stores permanent identity and clears bootstrap token.
- [ ] Test failed enrollment does not partially persist credentials.
- [ ] Implement transactional local persistence order: validate response, store secret, store non-secret config, erase bootstrap token.
- [ ] Add UI for server URL + enrollment code.
- [ ] Commit `feat: enroll enterprise devices by code`.

### Task 6: Employee Login / Self-Enrollment

**Files:**
- Create: `flutter/lib/enterprise/auth/login_page.dart`
- Create: `flutter/lib/enterprise/auth/account_session.dart`
- Extend Rust API/enrollment service.
- Test: login/session controller tests.

**Interfaces:**
- Produces authenticated account session and optional self-enrollment flow.

- [ ] Test valid login, disabled account, tenant resolution failure, and logout.
- [ ] Test self-enrollment is blocked when server says user lacks `devices.enroll`.
- [ ] Implement refresh/session token storage using secure storage when present.
- [ ] Commit `feat: add enterprise employee sign in`.

### Task 7: Gateway Connection and Heartbeat

**Files:**
- Create: `src/enterprise/gateway.rs`
- Create: `src/enterprise/heartbeat.rs`
- Create: `flutter/lib/enterprise/gateway/gateway_state.dart`
- Test: Rust WebSocket integration tests against local test Gateway.

**Interfaces:**
- Produces Gateway states: disconnected, connecting, authenticated, degraded.
- Sends v1 hello and 30-second heartbeat by default.

- [ ] Test authenticated connection using permanent credential.
- [ ] Test exponential reconnect backoff with jitter.
- [ ] Test heartbeat includes endpoint/client/policy metadata but no secrets.
- [ ] Test Gateway failure does not disable LAN-only direct remote control.
- [ ] Commit `feat: connect enterprise client to device gateway`.

### Task 8: Policy Fetch and Apply

**Files:**
- Create: `src/enterprise/policy.rs`
- Create: `flutter/lib/enterprise/policy/enterprise_policy.dart`
- Modify only established config setters required to apply approved policy values.
- Test: policy parsing/application tests.

**Interfaces:**
- Consumes server effective policy version/settings.
- Applies unattended access, clipboard/file transfer permission, LAN port/CIDR, heartbeat settings within safe bounds.

- [ ] Test unknown policy fields are ignored safely.
- [ ] Test heartbeat interval outside server/client safety bounds is clamped/rejected.
- [ ] Test `policy_changed` triggers HTTPS fetch, not arbitrary Gateway payload execution.
- [ ] Commit `feat: apply enterprise device policies`.

### Task 9: Cloud Device Repository and Flutter Device List

**Files:**
- Create: `flutter/lib/enterprise/devices/enterprise_device.dart`
- Create: `flutter/lib/enterprise/devices/device_repository.dart`
- Create: `flutter/lib/enterprise/ui/enterprise_devices_page.dart`
- Integrate a new enterprise section into the existing desktop navigation/home page with the smallest diff.
- Test: repository and widget tests.

**Interfaces:**
- Displays name, hostname, platform, online/offline, department/group, last seen.

- [ ] Test online and offline projection.
- [ ] Test tenant-scoped cloud devices are visually separate from Nearby LAN Devices.
- [ ] Implement My Devices, All Devices, Favorites/Recent placeholders only where backed by current API; avoid speculative features.
- [ ] Verify existing LAN screen remains reachable.
- [ ] Commit `feat: show enterprise managed devices`.

### Task 10: Authorization-Before-Connect

**Files:**
- Create: `src/enterprise/remote_authorization.rs`
- Modify the narrow existing Flutter/Rust connection entry point used when a cloud-managed device is selected.
- Test: authorization integration tests with the existing connection initiation abstracted/injected where possible.

**Interfaces:**
- Consumes target device ID + requested capabilities.
- Produces approved endpoint/fingerprint/capabilities before invoking the existing direct connection function.

- [ ] Test denied authorization never calls existing direct-connect entry point.
- [ ] Test approved authorization calls direct-connect exactly once with returned endpoint.
- [ ] Test timeout/API outage shows a managed-session error without changing LAN direct flow.
- [ ] Preserve existing device fingerprint verification.
- [ ] Commit `feat: authorize enterprise remote connections`.

### Task 11: Remote Session Audit Events

**Files:**
- Create: `src/enterprise/session_audit.rs`
- Modify managed connection lifecycle hooks only.
- Test: session lifecycle tests.

**Interfaces:**
- Sends started/ended/failed lifecycle events for `authorization_id`.

- [ ] Test session started is emitted only after connection reaches the chosen connected milestone.
- [ ] Test ended event is best-effort and does not hang disconnect.
- [ ] Ensure no screen, clipboard, keystroke, or file contents are included.
- [ ] Commit `feat: report managed remote session lifecycle`.

### Task 12: Preconfigured Installer Bootstrap

**Files:**
- Create: `src/enterprise/bootstrap.rs`
- Add packaging integration under the repository's existing Windows/macOS build/package scripts rather than inventing a parallel packaging system.
- Test: bootstrap config parsing tests and packaging smoke checks.

**Interfaces:**
- Accepts `control_plane_url` + one-time `enrollment_token` from deployment configuration.

- [ ] Test first launch consumes bootstrap config once.
- [ ] Test successful enrollment removes token/bootstrap secret material.
- [ ] Test failed enrollment retains only what is required for retry and never converts bootstrap token into a permanent shared credential.
- [ ] Commit `feat: support enterprise deployment bootstrap`.

### Task 13: Performance Preset Model

**Files:**
- Create: `src/enterprise/performance.rs` or place in the existing remote-display settings module if that is the established ownership boundary.
- Create: `flutter/lib/enterprise/performance/performance_preset.dart`
- Add settings UI using existing remote quality controls.
- Test: preset mapping tests.

**Interfaces:**
- Enum: `balanced`, `low_latency`, `high_quality`.
- Produces desired encoder/quality/frame scheduling hints using existing codec/settings interfaces.

- [ ] Write tests that each preset maps deterministically to existing quality/codec knobs.
- [ ] Balanced uses current/default behavior as closely as possible.
- [ ] Low Latency prefers hardware codec when available and favors bounded frame queue depth.
- [ ] High Quality raises fidelity within existing safe limits but does not bypass bandwidth controls.
- [ ] Commit `feat: add remote performance presets`.

### Task 14: Low-Latency Adaptation

**Files:**
- Modify only the existing encoder/quality-control/frame-queue modules identified during implementation.
- Create focused adaptation helper if existing code lacks a clean boundary.
- Test: Rust unit tests for adaptation decisions plus existing codec tests.

**Interfaces:**
- Inputs: RTT, estimated throughput, packet/frame loss or congestion indicators, encode time, queue depth.
- Outputs: target bitrate/quality, target FPS or frame skipping, resolution/scale hint where existing engine supports it.

- [ ] Write test: growing frame queue in Low Latency reduces quality/frames before allowing unbounded latency growth.
- [ ] Write test: improved network conditions recover gradually rather than oscillating every sample.
- [ ] Write test: input/control traffic is not queued behind bulk file-transfer traffic where transport priority mechanisms exist.
- [ ] Implement the smallest adaptation layer using existing RustDesk/SubnetDesk codec controls rather than replacing the codec stack.
- [ ] Commit `perf: add low latency network adaptation`.

### Task 15: Enterprise/LAN Regression Suite

**Files:**
- Add integration tests around existing direct connection entry points.
- Add Flutter navigation/widget regressions.
- Add a manual QA checklist under `docs/enterprise-client-qa.md`.

- [ ] Test client with no enterprise config launches and behaves as current LAN-only SubnetDesk.
- [ ] Test enterprise API/Gateway unreachable does not block Nearby LAN/manual IP connection.
- [ ] Test managed connection still uses direct target endpoint after authorization.
- [ ] Run targeted Rust tests, full relevant workspace tests, `flutter test`, and `flutter analyze`.
- [ ] Build at least Windows and macOS desktop artifacts through existing workflow commands.
- [ ] Commit `test: protect lan and enterprise client flows`.

## Performance Acceptance

For Low Latency mode, acceptance is behavioral rather than promising one universal millisecond value because network distance dominates end-to-end delay. Under a controlled LAN benchmark, the mode must show no regression versus current direct SubnetDesk behavior and should minimize accumulated frame latency by dropping/degrading stale visual work rather than allowing a growing render queue. Under constrained bandwidth, mouse/keyboard interaction must remain usable while image quality adapts downward.

## Plan Acceptance

This plan is complete when one desktop client can enroll through all approved paths, stay online through Gateway, display cloud-managed devices, obtain authorization, start the existing peer-to-peer remote-control path, report session lifecycle, switch performance presets, and continue to support the original LAN-only workflow when cloud services are absent.