# Remote Grant Authorization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce short-lived, Ed25519-signed Remote Grants on enterprise-managed inbound SubnetDesk connections before `Connection::start(...)`, while preserving explicit LAN-only compatibility modes and keeping remote media traffic peer-to-peer.

**Architecture:** Laravel signs a compact JWS-like Remote Grant with an Ed25519 private key. The SubnetDesk target validates the exact signed bytes locally using a trusted public-key ring, checks tenant/device/time/capability/replay constraints, and only then permits the existing connection pipeline to start. The existing `LanClientHello.client_capabilities` field is used to negotiate enterprise-auth intent, avoiding a protobuf/submodule fork in phase one.

**Tech Stack:** Laravel 13 / PHP >= 8.3 with ext-sodium; Rust 1.75+; existing `sodiumoxide`; existing encrypted LAN `Stream`; serde/serde_json; existing SubnetDesk tests; PHPUnit/Pest in the Laravel control-plane repository.

**Spec:** `docs/superpowers/specs/2026-08-21-remote-grant-authorization-design.md`

## Global Constraints

- Remote Grant maximum lifetime is 60 seconds.
- Target-side enforcement is mandatory for `ENTERPRISE_ONLY`.
- Ed25519 private signing keys remain server-side; SubnetDesk stores public verification keys only.
- Existing video/audio/input/file payloads must not pass through Laravel or Device Gateway.
- Existing LAN behavior remains available only when effective access mode is `LAN_ONLY` or `HYBRID`.
- No plaintext signing private key, enrollment token, device secret, or Remote Grant may be written to application logs.
- Use existing `sodiumoxide` in SubnetDesk; do not add a second Ed25519 crate unless existing APIs prove insufficient.
- Do not modify the `hbb_common` submodule protocol for phase one. Use the existing `LanClientHello.client_capabilities` field and an encrypted enterprise pre-session frame.
- Follow TDD: every production-code behavior begins with a failing test and the failure is observed before implementation.
- Preserve the existing connection order: encrypted LAN handshake first, enterprise authorization second, `Connection::start(...)` last.

---

## File Structure

### SubnetDesk repository

Create:

- `src/enterprise/mod.rs` — enterprise module exports and top-level access-mode helpers.
- `src/enterprise/protocol.rs` — enterprise capability bits and encrypted pre-session challenge/request/result wire structures.
- `src/enterprise/remote_grant.rs` — compact token parsing, Ed25519 signature verification, payload validation primitives.
- `src/enterprise/replay_cache.rs` — bounded in-memory authorization replay cache.
- `src/enterprise/authorization.rs` — target-side policy, keyring, grant validation orchestration, and capability result.

Modify:

- `src/lib.rs` or the repository's root module declaration file — expose `enterprise` module.
- `src/lan_protocol.rs` — advertise/read client capability bits and return handshake metadata without changing protobuf definitions.
- `src/server.rs` — call target-side enterprise authorization between `server_handshake()` and `Connection::start()`; carry granted capability restrictions in `ConnectionMeta`.
- `src/server/connection.rs` — apply enterprise capability ceilings so later login/options cannot re-enable denied features.
- `src/client.rs` — add an enterprise connection entry point that requests the enterprise-auth capability and performs the encrypted pre-session authorization exchange; keep existing `Client::start` as the LAN-compatible path.
- `Cargo.toml` — no new crypto dependency expected; only modify if tests reveal an existing feature import is required.

### Laravel control-plane repository

Create:

- `config/remote_grants.php` — active signing key ID, private key source, public key set, lifetime/skew constants.
- `app/Security/RemoteGrant/RemoteGrantPayload.php` — typed immutable payload value object.
- `app/Security/RemoteGrant/RemoteGrantSigner.php` — JWS-like compact token signer using ext-sodium Ed25519.
- `app/Security/RemoteGrant/RemoteGrantKeyProvider.php` — loads signing key by active `kid`, validates key lengths, exposes public key metadata.
- `app/Http/Requests/CreateRemoteAuthorizationRequest.php` — validates target/source device IDs and requested capabilities.
- `app/Http/Controllers/Api/V1/RemoteAuthorizationController.php` — authorization endpoint and audit record creation.
- `tests/Unit/Security/RemoteGrantSignerTest.php` — deterministic signing/token format tests.
- `tests/Feature/Api/V1/RemoteAuthorizationTest.php` — tenant/role/resource/capability authorization tests.
- `tests/Fixtures/remote_grant_vector.json` — cross-language golden vector consumed by PHP and Rust tests.

Modify:

- `routes/api.php` — add `POST /api/v1/remote-authorizations`.
- relevant `Device`, `RemoteSession`, permission/policy classes from the control-plane foundation — resolve target endpoint/fingerprint and resource scope.

---

### Task 1: Define enterprise-auth negotiation without a protobuf fork

**Files:**
- Create: `src/enterprise/mod.rs`
- Create: `src/enterprise/protocol.rs`
- Modify: `src/lan_protocol.rs`
- Test: unit tests in `src/lan_protocol.rs` and `src/enterprise/protocol.rs`

**Interfaces:**
- Produces `pub const LAN_CAP_ENTERPRISE_AUTH_V1: u64 = 1 << 0`.
- Produces `pub const LAN_CAP_ENTERPRISE_AUTH_REQUESTED: u64 = 1 << 1`.
- Produces `LanHandshakeMeta { pub client_capabilities: u64 }` from server handshake.
- Produces `client_handshake_with_capabilities(stream: &mut Stream, client_capabilities: u64) -> ResultType<LanPeerIdentity>` while preserving `client_handshake(stream)` as the compatibility wrapper using `0`.
- Produces serde wire structures `EnterpriseAuthChallenge`, `EnterpriseAuthRequest`, `EnterpriseAuthResult` in `src/enterprise/protocol.rs`.

- [ ] **Step 1: Add failing negotiation tests**

Add tests proving the server observes capability bits and the existing compatibility wrapper sends zero capabilities:

```rust
#[test]
fn enterprise_capability_bits_are_distinct() {
    assert_ne!(
        crate::enterprise::protocol::LAN_CAP_ENTERPRISE_AUTH_V1,
        crate::enterprise::protocol::LAN_CAP_ENTERPRISE_AUTH_REQUESTED
    );
    assert_eq!(
        crate::enterprise::protocol::LAN_CAP_ENTERPRISE_AUTH_V1
            & crate::enterprise::protocol::LAN_CAP_ENTERPRISE_AUTH_REQUESTED,
        0
    );
}
```

Extend the loopback handshake test so the client uses `LAN_CAP_ENTERPRISE_AUTH_V1 | LAN_CAP_ENTERPRISE_AUTH_REQUESTED` and asserts the server-side returned `LanHandshakeMeta.client_capabilities` contains both bits.

- [ ] **Step 2: Run the targeted Rust tests and confirm failure**

Run:

```bash
cargo test lan_protocol::tests::enterprise_capability_bits_are_distinct --lib
cargo test lan_protocol::tests::loopback_handshake_reports_client_capabilities --lib
```

Expected: FAIL because the enterprise module, constants, capability-aware handshake, or returned metadata do not exist yet.

- [ ] **Step 3: Implement protocol constants and handshake metadata**

In `src/enterprise/protocol.rs` define:

```rust
pub const LAN_CAP_ENTERPRISE_AUTH_V1: u64 = 1 << 0;
pub const LAN_CAP_ENTERPRISE_AUTH_REQUESTED: u64 = 1 << 1;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EnterpriseAuthChallenge {
    pub protocol_version: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EnterpriseAuthRequest {
    pub protocol_version: u32,
    pub remote_grant: String,
    pub requested_capabilities: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EnterpriseAuthResult {
    pub protocol_version: u32,
    pub accepted: bool,
    pub authorization_id: Option<String>,
    pub granted_capabilities: Vec<String>,
    pub reason: Option<String>,
}
```

In `src/lan_protocol.rs` preserve the current `client_handshake(&mut Stream)` API as a wrapper and move implementation into `client_handshake_with_capabilities`. Set `LanClientHello.client_capabilities` from the passed value. Change the server-side handshake internals to return:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanHandshakeMeta {
    pub client_capabilities: u64,
}
```

The return value is created from `client_hello.client_capabilities` after protocol validation.

- [ ] **Step 4: Run handshake tests**

Run:

```bash
cargo test lan_protocol::tests --lib
```

Expected: PASS, including the existing encrypted payload tests.

- [ ] **Step 5: Commit the negotiation layer**

```bash
git add src/enterprise/mod.rs src/enterprise/protocol.rs src/lan_protocol.rs
git commit -m "feat: negotiate enterprise LAN authorization"
```

---

### Task 2: Implement the compact Remote Grant token format in Laravel

**Files:**
- Create: `config/remote_grants.php`
- Create: `app/Security/RemoteGrant/RemoteGrantPayload.php`
- Create: `app/Security/RemoteGrant/RemoteGrantKeyProvider.php`
- Create: `app/Security/RemoteGrant/RemoteGrantSigner.php`
- Test: `tests/Unit/Security/RemoteGrantSignerTest.php`
- Create: `tests/Fixtures/remote_grant_vector.json`

**Interfaces:**
- Produces token format: `base64url(header_json).base64url(payload_json).base64url(ed25519_signature)`.
- Signature input is the exact ASCII bytes `header_b64 + "." + payload_b64`; the Rust verifier never reserializes JSON to verify a signature.
- Header is exactly `{"alg":"Ed25519","kid":"<kid>","typ":"SDRG1"}` when generated by the signer.
- `RemoteGrantSigner::sign(RemoteGrantPayload $payload): string`.
- Payload includes `ver`, `authorization_id`, `tenant_id`, `user_id`, `source_device_id`, `target_device_id`, `capabilities`, `issued_at`, `expires_at`, `nonce`.

- [ ] **Step 1: Write failing Laravel unit tests for token structure and lifetime**

```php
it('signs a compact Ed25519 remote grant', function () {
    $payload = new RemoteGrantPayload(
        ver: 1,
        authorizationId: '01JTESTAUTH0000000000000000',
        tenantId: 12,
        userId: 381,
        sourceDeviceId: '00000000-0000-4000-8000-000000000001',
        targetDeviceId: '00000000-0000-4000-8000-000000000002',
        capabilities: ['clipboard', 'remote_control'],
        issuedAt: 1787302800,
        expiresAt: 1787302860,
        nonce: 'AAECAwQFBgcICQoLDA0ODw',
    );

    $token = app(RemoteGrantSigner::class)->sign($payload);
    expect(explode('.', $token))->toHaveCount(3);
});
```

Add a second test asserting a payload with `expiresAt - issuedAt > 60` throws `InvalidArgumentException` before signing.

- [ ] **Step 2: Run the tests and confirm failure**

Run:

```bash
php artisan test tests/Unit/Security/RemoteGrantSignerTest.php
```

Expected: FAIL because Remote Grant classes do not exist.

- [ ] **Step 3: Implement the signer using ext-sodium**

Use `sodium_crypto_sign_detached($signingInput, $secretKey)` and URL-safe base64 without padding. Load the active `kid` and base64-encoded Ed25519 secret key from environment-backed config; never store the private key in the repository.

`RemoteGrantPayload` constructor validates:

```text
ver == 1
expires_at > issued_at
expires_at - issued_at <= 60
nonce decodes to exactly 16 bytes
capabilities are unique and drawn from the supported capability set
```

Sort capabilities lexicographically before JSON encoding so generated payloads are stable for tests and audit comparison.

- [ ] **Step 4: Generate and freeze a cross-language golden vector**

Create `tests/Fixtures/remote_grant_vector.json` containing a fixed test seed/key pair, expected public key, fixed payload fields, expected compact token, and expected granted capability array. The secret key in this fixture is test-only and must be visibly labeled as non-production.

- [ ] **Step 5: Run unit tests**

```bash
php artisan test tests/Unit/Security/RemoteGrantSignerTest.php
```

Expected: PASS.

- [ ] **Step 6: Commit the signing implementation**

```bash
git add config/remote_grants.php app/Security/RemoteGrant tests/Unit/Security/RemoteGrantSignerTest.php tests/Fixtures/remote_grant_vector.json
git commit -m "feat: sign short-lived remote grants"
```

---

### Task 3: Implement Rust Remote Grant parsing and Ed25519 verification

**Files:**
- Create: `src/enterprise/remote_grant.rs`
- Modify: `src/enterprise/mod.rs`
- Test: unit tests in `src/enterprise/remote_grant.rs`
- Test fixture: copy the public, non-secret golden vector values into `src/enterprise/testdata/remote_grant_vector.json` or load an equivalent checked-in fixture under `tests/fixtures/`.

**Interfaces:**
- Produces `RemoteGrantPayload` matching the Laravel payload field names.
- Produces `RemoteGrantVerifier::verify(&self, token: &str, now_unix: i64) -> Result<VerifiedRemoteGrant, RemoteGrantError>`.
- `VerifiedRemoteGrant` exposes parsed payload and the signing `kid`.
- Signature verification uses the exact `header_b64.payload_b64` bytes from the token and the trusted key selected by `kid`.

- [ ] **Step 1: Write failing verifier tests**

Tests must cover:

```text
valid Laravel golden token passes
one-byte payload change fails signature
unknown kid fails
wrong alg/typ fails
expired token fails
token longer than 60 seconds fails
future issued_at beyond 30-second skew fails
malformed nonce fails
unsupported capability fails
```

Example shape:

```rust
#[test]
fn verifies_laravel_golden_vector() {
    let vector = load_vector();
    let verifier = verifier_from_vector(&vector);
    let verified = verifier.verify(&vector.token, vector.now).unwrap();
    assert_eq!(verified.payload.target_device_id, vector.target_device_id);
}
```

- [ ] **Step 2: Run tests and observe failure**

```bash
cargo test enterprise::remote_grant::tests --lib
```

Expected: FAIL because verifier types do not exist.

- [ ] **Step 3: Implement compact-token parser and signature verification**

Parsing rules:

1. split into exactly three dot-separated segments;
2. base64url-decode header and payload with no reliance on key ordering;
3. parse header and require `alg == "Ed25519"`, `typ == "SDRG1"`, known `kid`;
4. base64url-decode detached signature and require Ed25519 signature length;
5. verify detached signature against exact ASCII signing input made from the original first two segments;
6. parse payload only after signature validation;
7. apply lifetime/skew/nonce/capability structural checks.

Use existing `hbb_common::sodiumoxide::crypto::sign` APIs. Do not add another Ed25519 implementation.

- [ ] **Step 4: Run verifier tests**

```bash
cargo test enterprise::remote_grant::tests --lib
```

Expected: PASS.

- [ ] **Step 5: Commit verifier**

```bash
git add src/enterprise/mod.rs src/enterprise/remote_grant.rs tests/fixtures/remote_grant_vector.json
git commit -m "feat: verify signed remote grants"
```

---

### Task 4: Add bounded replay protection and local enterprise policy

**Files:**
- Create: `src/enterprise/replay_cache.rs`
- Create: `src/enterprise/authorization.rs`
- Modify: `src/enterprise/mod.rs`
- Test: unit tests in both new files

**Interfaces:**
- Produces `enum EnterpriseAccessMode { LanOnly, Hybrid, EnterpriseOnly }`.
- Produces `enum EnterpriseCapability { RemoteControl, Clipboard, FileTransfer, Audio }`.
- Produces `ReplayCache::consume(&mut self, authorization_id: &str, now_unix: i64, retain_until: i64) -> bool`, returning `true` only for the first consumption.
- Produces `AuthorizationContext { tenant_id, device_id, access_mode, allowed_capabilities, trusted_keys }`.
- Produces `authorize_remote_grant(context, verifier, replay_cache, token, requested_capabilities, now_unix) -> Result<AuthorizedEnterpriseSession, AuthorizationError>`.

- [ ] **Step 1: Write replay-cache failing tests**

Cover first-use acceptance, second-use rejection, expiration eviction, and hard size bound. Use a maximum cache size of 4096 entries for phase one; evict expired entries first, then oldest expiry when the bound is exceeded.

- [ ] **Step 2: Write authorization failing tests**

Cover:

```text
wrong target device rejected
wrong tenant rejected
requested capabilities must be subset of signed capabilities
signed capabilities must be subset of local policy capabilities
authorization_id is consumed only after all validation passes
second valid use rejected as replay
```

- [ ] **Step 3: Run tests and observe failure**

```bash
cargo test enterprise::replay_cache::tests --lib
cargo test enterprise::authorization::tests --lib
```

Expected: FAIL because the components do not exist.

- [ ] **Step 4: Implement replay cache and authorization orchestration**

Keep the replay cache in memory behind a `Mutex`. Store only authorization ID plus expiry/consumption timestamps; never store the full token.

`AuthorizedEnterpriseSession` must contain exactly the data required downstream:

```rust
pub struct AuthorizedEnterpriseSession {
    pub authorization_id: String,
    pub user_id: u64,
    pub capabilities: std::collections::BTreeSet<EnterpriseCapability>,
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test enterprise::replay_cache::tests --lib
cargo test enterprise::authorization::tests --lib
```

Expected: PASS.

- [ ] **Step 6: Commit local authorization policy**

```bash
git add src/enterprise/mod.rs src/enterprise/replay_cache.rs src/enterprise/authorization.rs
git commit -m "feat: enforce remote grant policy and replay protection"
```

---

### Task 5: Enforce enterprise authorization before `Connection::start(...)`

**Files:**
- Modify: `src/server.rs`
- Modify: `src/server/connection.rs`
- Modify: `src/enterprise/protocol.rs`
- Test: unit/integration tests near `src/server.rs` and `src/enterprise/protocol.rs`

**Interfaces:**
- `ConnectionMeta` gains an optional enterprise authorization result/capability ceiling.
- `create_lan_connection(...)` receives `LanHandshakeMeta`, determines effective access mode, performs enterprise pre-session exchange when required/requested, and only then calls `Connection::start(...)`.
- `Connection` capability booleans can be reduced by `ConnectionMeta`; normal login/options may never elevate beyond that ceiling.

- [ ] **Step 1: Add failing server-gate tests**

Create loopback cases proving:

```text
ENTERPRISE_ONLY + client without ENTERPRISE_AUTH_REQUESTED -> rejected before Connection::start
ENTERPRISE_ONLY + invalid grant -> rejected
ENTERPRISE_ONLY + valid grant -> accepted
HYBRID + legacy client -> existing path preserved
HYBRID + enterprise-request client -> grant path used
LAN_ONLY + legacy client -> existing path preserved
```

Use a test seam/counter immediately before `Connection::start` so tests can prove the rejected cases never cross the gate without starting full capture services.

- [ ] **Step 2: Run the server-gate tests and observe failure**

```bash
cargo test server::tests::enterprise_only_rejects_legacy_client --lib
cargo test server::tests::enterprise_only_accepts_valid_remote_grant --lib
```

Expected: FAIL because server enforcement is absent.

- [ ] **Step 3: Implement encrypted pre-session exchange**

After `server_handshake()` completes:

```text
if access_mode == LAN_ONLY:
    proceed legacy
else if access_mode == HYBRID and ENTERPRISE_AUTH_REQUESTED bit is absent:
    proceed legacy
else:
    require ENTERPRISE_AUTH_V1 and ENTERPRISE_AUTH_REQUESTED bits
    send EnterpriseAuthChallenge over the already encrypted Stream
    read EnterpriseAuthRequest
    validate Remote Grant locally
    send coarse EnterpriseAuthResult
    on failure return error before Connection::start
```

This challenge is a compatibility refinement: an enterprise client waits for the target challenge before sending a Remote Grant, so a new controller does not accidentally send enterprise JSON into an older SubnetDesk server that does not implement this protocol.

- [ ] **Step 4: Apply capability ceilings in `ConnectionMeta`**

Extend `ConnectionMeta` with enterprise-allowed booleans or an enterprise capability structure. When constructing `Connection`, initialize `keyboard`, `clipboard`, `audio`, and `file` so they cannot exceed the signed/local-policy capability set. Later session/login messages may turn a capability off but may never turn a denied capability back on.

Mapping for phase one:

```text
remote_control -> keyboard/input enabled
clipboard -> clipboard enabled
file_transfer -> file enabled
audio -> audio enabled
```

- [ ] **Step 5: Run server and connection tests**

```bash
cargo test server::tests --lib
cargo test server::connection::tests --lib
```

Expected: PASS with existing LAN authentication tests still green.

- [ ] **Step 6: Commit target-side enforcement**

```bash
git add src/server.rs src/server/connection.rs src/enterprise/protocol.rs
git commit -m "feat: enforce remote grants before remote session start"
```

---

### Task 6: Add the enterprise controller connection path

**Files:**
- Modify: `src/client.rs`
- Modify: `src/lan_protocol.rs`
- Modify: `src/enterprise/protocol.rs`
- Test: targeted tests in `src/client.rs` / `src/lan_protocol.rs`

**Interfaces:**
- Preserve `Client::start(...)` for existing LAN behavior.
- Add `Client::start_enterprise(peer, conn_type, interface, remote_grant, requested_capabilities)` or an equivalent narrowly scoped wrapper that does not alter legacy callers.
- Enterprise path sends `LAN_CAP_ENTERPRISE_AUTH_V1 | LAN_CAP_ENTERPRISE_AUTH_REQUESTED` in `LanClientHello`.
- After encrypted handshake, enterprise path waits for `EnterpriseAuthChallenge`, sends `EnterpriseAuthRequest`, requires an accepted `EnterpriseAuthResult`, then returns the same `(Stream, Vec<u8>, String)` tuple expected by the current session pipeline.

- [ ] **Step 1: Write failing controller tests**

Cover:

```text
enterprise path sets both capability bits
enterprise path refuses target that never sends challenge
enterprise path sends grant only after encrypted handshake/challenge
rejected result surfaces a generic authorization error
legacy Client::start still sends zero enterprise-request bits and behaves unchanged
```

- [ ] **Step 2: Run tests and observe failure**

```bash
cargo test client::tests::enterprise_connection_requires_target_challenge --lib
cargo test client::tests::legacy_connection_does_not_request_enterprise_auth --lib
```

Expected: FAIL because the enterprise controller path is absent.

- [ ] **Step 3: Implement enterprise connection wrapper**

Use the existing endpoint candidate/fingerprint logic from `Client::start`; factor only the minimum shared internal helper necessary to avoid duplicating the entire connection loop. Do not refactor unrelated client code.

Set a short challenge timeout for the enterprise pre-session exchange (initial target: 2 seconds, separate from screen/session timeouts). If an enterprise-managed cloud connection reaches a target that lacks the challenge, fail closed with a clear local error such as `Target does not support enterprise authorization`; do not silently fall back to LAN credentials.

- [ ] **Step 4: Run client and LAN protocol tests**

```bash
cargo test client::tests --lib
cargo test lan_protocol::tests --lib
```

Expected: PASS.

- [ ] **Step 5: Commit enterprise controller path**

```bash
git add src/client.rs src/lan_protocol.rs src/enterprise/protocol.rs
git commit -m "feat: connect with enterprise remote authorization"
```

---

### Task 7: Implement Laravel `POST /api/v1/remote-authorizations`

**Files:**
- Create: `app/Http/Requests/CreateRemoteAuthorizationRequest.php`
- Create: `app/Http/Controllers/Api/V1/RemoteAuthorizationController.php`
- Modify: `routes/api.php`
- Test: `tests/Feature/Api/V1/RemoteAuthorizationTest.php`

**Interfaces:**
- Consumes `RemoteGrantSigner::sign(RemoteGrantPayload): string`.
- Returns `authorization_id`, `expires_at`, target endpoint/fingerprint, granted capabilities, and `remote_grant`.
- Creates/updates the durable `remote_sessions` authorization record before returning the token.

- [ ] **Step 1: Write failing feature tests for tenant/resource authorization**

At minimum test:

```text
unauthenticated -> 401
cross-tenant target -> 404 or 403 according to API convention, never target metadata
user without devices.remote_control -> 403
disabled target -> 409 or 403 according to API convention
requested file_transfer removed/denied when policy forbids it
valid request -> 201/200 with 60-second signed remote_grant
audit/remote_sessions row contains authorization_id but not token or signing secret
```

Example request:

```php
$response = $this->actingAs($operator)->postJson('/api/v1/remote-authorizations', [
    'target_device_id' => $target->uuid,
    'source_device_id' => $source->uuid,
    'capabilities' => ['remote_control', 'clipboard'],
]);

$response->assertOk()
    ->assertJsonPath('target.device_id', $target->uuid)
    ->assertJsonStructure(['authorization_id', 'expires_at', 'remote_grant']);
```

- [ ] **Step 2: Run feature tests and observe failure**

```bash
php artisan test tests/Feature/Api/V1/RemoteAuthorizationTest.php
```

Expected: FAIL because endpoint/controller do not exist.

- [ ] **Step 3: Implement request validation and scope checks**

`CreateRemoteAuthorizationRequest` accepts only known capability strings and deduplicates them. Controller resolves both devices within the authenticated tenant scope, checks role/resource policy, intersects requested capabilities with tenant/device policy, requires `remote_control` for a normal remote-control connection, and generates:

```text
issued_at = current unix second
expires_at = issued_at + 60
authorization_id = ULID
nonce = 16 cryptographically random bytes encoded base64url
```

The controller stores the authorization record and signs the grant only after authorization succeeds.

- [ ] **Step 4: Run feature and unit tests**

```bash
php artisan test tests/Feature/Api/V1/RemoteAuthorizationTest.php
php artisan test tests/Unit/Security/RemoteGrantSignerTest.php
```

Expected: PASS.

- [ ] **Step 5: Commit endpoint**

```bash
git add app/Http/Requests/CreateRemoteAuthorizationRequest.php app/Http/Controllers/Api/V1/RemoteAuthorizationController.php routes/api.php tests/Feature/Api/V1/RemoteAuthorizationTest.php
git commit -m "feat: authorize enterprise remote sessions"
```

---

### Task 8: Add signing-key rotation and trusted-key delivery

**Files:**
- Laravel: `config/remote_grants.php`, `app/Security/RemoteGrant/RemoteGrantKeyProvider.php`
- Laravel: add authenticated effective-policy/public-key endpoint in the enterprise policy API
- SubnetDesk: `src/enterprise/authorization.rs` and enterprise policy/key persistence module from the enterprise-client plan
- Tests: Laravel feature tests plus Rust keyring tests

**Interfaces:**
- Control plane exposes public verification keys as `{ kid, alg, public_key_base64, not_before, retire_after }` through authenticated enterprise policy/bootstrap data.
- SubnetDesk keyring accepts multiple active public keys and rejects unknown `kid`.
- Signing service signs only with one configured active `kid` at a time.

- [ ] **Step 1: Write failing key-rotation tests**

Prove:

```text
old + new public keys can coexist
new active signer emits new kid
old grant continues to verify until old key removed and token lifetime passes
unknown kid is rejected locally
private key never appears in public-key endpoint
```

- [ ] **Step 2: Run tests and observe failure**

Run the targeted PHP and Rust test files.

- [ ] **Step 3: Implement keyring delivery and rotation behavior**

Public keys are policy/bootstrap data and may be cached locally. Do not fetch keys synchronously during an inbound authorization attempt. An unknown `kid` fails closed; the background enterprise policy refresh is responsible for obtaining newly published keys.

- [ ] **Step 4: Run rotation tests**

Expected: PASS.

- [ ] **Step 5: Commit rotation support**

Commit Laravel and SubnetDesk changes separately in their respective repositories with focused messages.

---

### Task 9: End-to-end compatibility, security, and latency verification

**Files:**
- Add integration tests under the existing Rust test locations.
- Add Laravel feature/integration tests under `tests/Feature`.
- Add documentation to the enterprise deployment/test guide.

**Interfaces:**
- Valid enterprise path must add authorization work only during connection setup.
- Existing video/audio/input data flow remains unchanged after `Connection::start(...)`.

- [ ] **Step 1: Add an end-to-end valid enterprise connection test**

Test sequence:

```text
Laravel-style fixture token -> encrypted LAN handshake -> capability negotiation -> challenge -> grant verification -> capability ceiling -> connection gate opens
```

Use loopback streams and test seams rather than starting a real desktop capture session.

- [ ] **Step 2: Add negative compatibility matrix tests**

Matrix:

```text
new controller / new target / ENTERPRISE_ONLY / valid grant -> allow
new controller / new target / ENTERPRISE_ONLY / no grant -> deny
old controller / new target / ENTERPRISE_ONLY -> deny
old controller / new target / HYBRID -> legacy allowed
old controller / new target / LAN_ONLY -> legacy allowed
new enterprise controller / old target -> fail closed, no silent LAN fallback
wrong-tenant grant -> deny
wrong-target grant -> deny
replayed grant -> deny
expired grant -> deny
```

- [ ] **Step 3: Add capability ceiling tests**

A token signed only for `remote_control` must not permit clipboard, file transfer, or audio even if the controller requests/enables them later in normal session messages.

- [ ] **Step 4: Add connection-setup latency benchmark**

Benchmark only local verification cost, excluding network RTT. Execute at least 10,000 Remote Grant verifications with a warm keyring and report p50/p95. Acceptance target for local grant verification: p95 under 5 ms on a typical modern desktop CPU. This is not a media-path benchmark.

- [ ] **Step 5: Run complete verification suites**

SubnetDesk:

```bash
cargo test --lib
```

Control plane:

```bash
php artisan test
```

Expected: all tests pass.

- [ ] **Step 6: Verify no accidental media-path changes**

Review diff and confirm no Remote Grant/Laravel/Gateway call was added inside screen-frame, audio-frame, input-event, clipboard-payload, or file-block hot loops.

- [ ] **Step 7: Commit integration tests and documentation**

```bash
git add <only the integration-test and documentation paths>
git commit -m "test: verify enterprise remote authorization end to end"
```

---

## Self-Review Checklist

Before implementation is considered complete, verify the plan/spec pair covers all of these requirements:

- target-side grant enforcement before `Connection::start`;
- Ed25519 signature and `kid` rotation;
- 60-second maximum lifetime and 30-second future skew tolerance;
- tenant and target device binding;
- capability subset enforcement;
- in-memory replay protection;
- `LAN_ONLY`, `HYBRID`, `ENTERPRISE_ONLY` behavior;
- old-client behavior in all three modes;
- new enterprise client fails closed against an old target;
- no synchronous Laravel/Gateway dependency during target verification;
- no media-path proxying or per-frame authorization work;
- cross-language golden vector proves PHP signer and Rust verifier compatibility;
- complete Laravel tenant/resource permission tests;
- local authorization latency benchmark.

## Recommended Execution Order

Implement Tasks 1–6 in SubnetDesk and Tasks 2/7 in the Laravel control plane in parallel only after the shared token fixture/schema is fixed. Task 8 follows once both sides verify the same token format. Task 9 is the release gate.
