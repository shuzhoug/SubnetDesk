# SubnetDesk Enterprise Remote Grant Authorization Design

Date: 2026-08-21

## Status and precedence

This specification is part of the SubnetDesk Enterprise Management design and is approved for phase one.

It **supersedes** the earlier phase-one deferral of cryptographically binding cloud authorization into the target-side connection path. Phase one now requires target-side enforcement for enterprise-managed connections.

## 1. Goal

Require every enterprise-managed remote connection to present a short-lived, server-signed authorization grant that the target SubnetDesk client verifies locally before the normal remote-control session starts.

The design must:

- prevent a modified controller client from bypassing Laravel authorization;
- avoid placing Laravel or Device Gateway in the continuous screen/audio/input data path;
- add minimal latency to connection setup only;
- preserve legacy LAN-only behavior when local policy explicitly permits it;
- support capability-level restrictions such as remote control, clipboard, file transfer, and audio;
- support signing-key rotation without forcing an immediate fleet-wide client upgrade.

## 2. Existing integration point

Current LAN server flow accepts a TCP connection in `src/lan_server.rs`, applies source-network checks, then calls `crate::server::create_lan_connection(...)`.

`create_lan_connection(...)` in `src/server.rs` currently performs:

1. `crate::lan_protocol::server_handshake(&mut stream).await?`;
2. allocate a connection ID;
3. call `Connection::start(...)`.

Enterprise authorization must run **after the secure/identity LAN handshake is established and before `Connection::start(...)` begins the normal remote-control services**.

Conceptually:

```text
server_handshake()
enterprise_authorize()
Connection::start()
```

No screen capture, input, clipboard, audio, or file-transfer service becomes available until enterprise authorization has succeeded when enterprise policy requires it.

## 3. Connection access modes

Each enrolled target device has an effective access mode controlled by enterprise policy.

### `LAN_ONLY`

Existing SubnetDesk LAN authentication behavior remains available. Enterprise Remote Grant is not required.

This mode is intended for non-enterprise or explicitly local-only deployments.

### `HYBRID`

Both paths are permitted:

- a valid enterprise Remote Grant; or
- the existing LAN authentication path, subject to existing local credentials and CIDR restrictions.

This mode is intended for organizations that want centrally managed access while retaining local emergency/maintenance access.

### `ENTERPRISE_ONLY`

Every managed inbound remote-control session must present a valid Remote Grant. A connection without a valid Remote Grant is rejected before `Connection::start(...)`.

Recommended default for enterprise-enrolled devices: `ENTERPRISE_ONLY`.

## 4. Authorization flow

```text
Operator selects target device
        |
        v
Controller calls Laravel Remote Authorization API
        |
        v
Laravel checks tenant/user/role/resource scope/device/policy
        |
        v
Laravel signs short-lived Remote Grant
        |
        v
Controller opens normal SubnetDesk connection to target
        |
        v
Existing LAN secure/identity handshake completes
        |
        v
Controller sends EnterpriseAuthRequest containing Remote Grant
        |
        v
Target verifies grant locally
        |
   +----+----+
   |         |
 reject    accept
   |         |
close        v
       Connection::start(...)
```

Laravel and Device Gateway are not contacted synchronously by the target during grant verification.

## 5. Signing algorithm and keys

Use Ed25519 signatures for Remote Grants.

The signing private key is held only by the Laravel control plane or a dedicated signing service under control-plane trust.

Target clients receive only trusted public keys.

Each key has a stable key identifier (`kid`). Example:

```text
rg-2026-01
```

The verifier maintains a keyring mapping `kid` to an Ed25519 public key.

### Key rotation

Rotation sequence:

1. distribute the new public key while retaining the old public key;
2. update the signing service to sign new grants with the new private key and `kid`;
3. wait longer than the maximum Remote Grant lifetime;
4. remove the old public key after operational confirmation.

Clients must reject unknown `kid` values rather than attempting an online fallback that weakens enforcement.

## 6. Remote Grant format

Phase one uses a compact signed envelope with a canonical serialized payload.

Logical payload:

```json
{
  "ver": 1,
  "authorization_id": "01J...",
  "tenant_id": 12,
  "user_id": 381,
  "source_device_id": "device-a-uuid",
  "target_device_id": "device-b-uuid",
  "capabilities": ["remote_control", "clipboard"],
  "issued_at": 1787302800,
  "expires_at": 1787302860,
  "nonce": "128-bit-random-value"
}
```

Envelope metadata includes:

```json
{
  "alg": "Ed25519",
  "kid": "rg-2026-01"
}
```

The implementation must define one canonical byte representation for signing and verification. JSON object ordering must never be relied upon implicitly. Either canonical JSON or a deterministic binary encoding may be used, but both Laravel and Rust implementations must share test vectors proving byte-for-byte compatibility.

## 7. Grant lifetime

Initial maximum grant lifetime: **60 seconds**.

Validation rules:

- `expires_at > now`;
- `expires_at - issued_at <= 60 seconds`;
- tolerate at most 30 seconds of clock skew when validating `issued_at` being in the future;
- reject grants whose `issued_at` is older than the accepted grant lifetime plus skew allowance even if other fields are malformed in a way that could otherwise extend validity.

A grant authorizes creation of one new remote session. It is not a reusable long-lived credential.

## 8. Mandatory target-side validation

The target must reject a grant unless every required condition passes:

1. supported `ver`;
2. recognized `alg` and `kid`;
3. valid Ed25519 signature over the canonical payload;
4. `target_device_id` equals the local enrolled enterprise device ID;
5. `tenant_id` equals the local enrolled tenant ID;
6. `expires_at` and `issued_at` satisfy lifetime/skew checks;
7. `authorization_id` is syntactically valid and non-empty;
8. `nonce` meets the required random-value format;
9. requested session behavior is a subset of the signed `capabilities`;
10. the local effective enterprise policy permits the access mode and granted capabilities;
11. the authorization has not already been consumed for another new session.

Any failure results in a closed connection before normal remote-control services start.

## 9. Replay protection

The target maintains an in-memory consumed-authorization cache keyed by `authorization_id`.

Behavior:

- first successfully validated use: mark consumed and allow session startup;
- later connection attempt using the same `authorization_id`: reject as replay;
- cache retention: at least 120 seconds from consumption;
- cache is bounded; expired entries are evicted;
- restarting the target may clear this in-memory cache.

The short 60-second lifetime limits risk across restarts. A later hardening phase may persist replay state if needed for higher-assurance environments.

The cache records only non-secret identifiers and expiry metadata.

## 10. Enterprise authentication protocol messages

After the existing secure LAN handshake, the controller sends an enterprise authorization message when attempting the enterprise path.

Logical request:

```json
{
  "type": "enterprise_auth_request",
  "protocol_version": 1,
  "remote_grant": "signed-envelope-bytes",
  "requested_capabilities": ["remote_control", "clipboard"]
}
```

Logical response on success:

```json
{
  "type": "enterprise_auth_result",
  "accepted": true,
  "authorization_id": "01J...",
  "granted_capabilities": ["remote_control", "clipboard"]
}
```

Logical response on failure may carry a coarse reason code:

```json
{
  "type": "enterprise_auth_result",
  "accepted": false,
  "reason": "invalid_authorization"
}
```

Do not expose cryptographic verification detail to an unauthenticated remote peer. Detailed failure diagnostics belong only in local logs with secrets removed.

Suggested local reason categories:

- `missing_authorization`;
- `invalid_signature`;
- `unknown_signing_key`;
- `wrong_target`;
- `wrong_tenant`;
- `expired`;
- `not_yet_valid`;
- `replay`;
- `capability_denied`;
- `policy_denied`;
- `malformed`.

## 11. Capability enforcement

Initial capability vocabulary:

- `remote_control`;
- `clipboard`;
- `file_transfer`;
- `audio`.

The target computes the effective capabilities as the intersection of:

1. capabilities signed into the Remote Grant;
2. capabilities requested by the controller;
3. capabilities allowed by local effective policy.

A capability not in the intersection is disabled for that connection.

Granting `remote_control` does not implicitly grant `file_transfer` or `clipboard`.

The authorization result must be propagated into connection metadata so later service subscription logic cannot re-enable a denied capability merely because a modified controller requests it.

## 12. Rust module boundaries

Add an isolated enterprise authorization module rather than spreading business rules through existing remote-control services.

Recommended structure:

```text
src/enterprise/
  mod.rs
  remote_grant.rs
  authorization.rs
  replay_cache.rs
  policy.rs
  signing_keys.rs
```

Responsibilities:

### `remote_grant.rs`

- grant data types;
- canonical decoding;
- structural validation;
- capability representation.

### `signing_keys.rs`

- trusted keyring;
- `kid` lookup;
- Ed25519 signature verification interface;
- key rotation support.

### `replay_cache.rs`

- bounded consumed-authorization cache;
- atomic consume operation;
- expiry cleanup.

### `policy.rs`

- target access mode (`LAN_ONLY`, `HYBRID`, `ENTERPRISE_ONLY`);
- locally effective capability rules.

### `authorization.rs`

- orchestrate signature/time/identity/replay/policy checks;
- return an `AuthorizedSession` object containing authorization ID, requester identity metadata, and effective capabilities.

## 13. Existing-code touch points

Phase-one implementation should minimize changes outside the new enterprise module.

Expected touch points:

### `src/server.rs`

Modify `create_lan_connection(...)` so it can run the enterprise authorization stage after `server_handshake(...)` and before `Connection::start(...)` when required by policy.

The result should be carried into connection metadata.

### `src/lan_server.rs`

No enterprise business logic belongs here. It should continue accepting sockets, enforcing source CIDRs, and delegating connection setup to `create_lan_connection(...)`.

### LAN protocol module

Extend the post-handshake protocol with the enterprise authorization request/result messages. Transport parsing belongs here; grant policy decisions do not.

### Connection metadata / permission setup

Extend per-connection metadata to carry effective enterprise capabilities. Existing service subscription permission logic must consume this state when enabling clipboard, file transfer, audio, and control-related services.

## 14. Laravel responsibilities

`POST /api/v1/remote-authorizations` remains the control-plane entry point.

After existing tenant/RBAC/resource-scope/policy checks pass, Laravel creates a `remote_sessions` authorization record and signs a Remote Grant containing the approved fields.

Response includes:

```json
{
  "authorization_id": "01J...",
  "expires_at": "2026-08-21T09:01:00Z",
  "remote_grant": "signed-envelope",
  "target": {
    "device_id": "device-b-uuid",
    "endpoint": "10.10.1.20:21118",
    "fingerprint": "..."
  },
  "capabilities": ["remote_control", "clipboard"]
}
```

Laravel must never place signing private keys in a client response.

Signing-key identifiers and public-key distribution metadata are not secret.

## 15. Gateway responsibilities

Device Gateway is not in the grant validation data path.

It may deliver:

- updated enterprise policy versions;
- signing-public-key/keyring updates;
- device state changes.

A Gateway outage must not terminate a remote session that already passed target-side authorization and is using the direct SubnetDesk data path.

## 16. Failure behavior

### Laravel unavailable before authorization

A new `ENTERPRISE_ONLY` session cannot obtain a grant and therefore cannot start.

### Laravel becomes unavailable after a valid grant is issued

The target can validate the still-valid signed grant locally without contacting Laravel.

### Gateway unavailable

Already enrolled targets retain cached effective policy and trusted signing keys. A valid non-expired grant can still be verified locally if the connection path is otherwise reachable.

### Signing-key cache unavailable or corrupted

Fail closed for enterprise authorization. `HYBRID` may still permit the explicitly configured legacy LAN path; `ENTERPRISE_ONLY` does not.

### Existing remote session after cloud failure

Do not terminate an already established direct session solely because Laravel, Redis, or Gateway becomes temporarily unavailable.

## 17. Auditing

Laravel records the authorization decision when it signs the grant.

The controller reports session start/end as already planned, but target-side enforcement also emits local enterprise authorization events that can be forwarded through Gateway when available.

Target-side event fields:

- `authorization_id`;
- `target_device_id`;
- timestamp;
- accepted/denied;
- coarse denial category;
- effective capabilities on accepted sessions.

No Remote Grant raw bytes, signing keys, screen contents, clipboard contents, keystrokes, or file payloads are logged.

A later phase may reconcile controller and target session events server-side for stronger audit completeness.

## 18. Performance requirements

Remote Grant validation happens only during connection establishment.

It must not be executed per video frame, input event, audio packet, clipboard update, or file block.

After authorization succeeds, the direct remote desktop data path remains unchanged except for per-connection capability gates.

Target requirement: local authorization validation should be operationally negligible compared with network connection setup and media initialization; no synchronous control-plane request is permitted during target verification.

## 19. Compatibility rules

- Existing non-enrolled SubnetDesk LAN deployments retain their existing behavior.
- Enrolled devices receive an explicit access-mode policy.
- New enterprise controller to old target: enterprise security cannot be guaranteed; production enterprise workflows must require a target version supporting this specification.
- Old controller to `ENTERPRISE_ONLY` target: rejected because it cannot provide Remote Grant authorization.
- Old controller to `HYBRID` target: may use the legacy LAN path only if existing credentials/CIDR policy allow it.

The admin UI must expose target client version/support status so administrators can identify devices not yet capable of enforced enterprise authorization.

## 20. Test requirements

### Cross-language signature vectors

Laravel and Rust test fixtures must share fixed test vectors covering:

- valid signature;
- modified target device ID;
- modified capability;
- modified expiry;
- wrong public key;
- unknown `kid`.

### Rust authorization tests

Required behaviors:

- accepts a valid grant for the local tenant/device;
- rejects wrong target;
- rejects wrong tenant;
- rejects expired grant;
- rejects excessive lifetime;
- rejects unsupported version;
- rejects replay;
- rejects capability escalation;
- applies local policy intersection;
- `ENTERPRISE_ONLY` rejects missing grant;
- `HYBRID` preserves explicitly permitted legacy LAN path.

### Integration tests

At minimum:

1. authorized enterprise connection reaches `Connection::start(...)`;
2. missing/invalid grant never reaches `Connection::start(...)` in `ENTERPRISE_ONLY`;
3. a grant cannot be reused to create a second connection;
4. clipboard/file transfer remain unavailable when omitted from signed capabilities;
5. existing LAN mode continues to work under `LAN_ONLY` and the legacy branch of `HYBRID`.

## 21. Phase-one acceptance criteria

This feature is complete when:

1. Laravel signs a 60-second Ed25519 Remote Grant after normal enterprise permission checks;
2. the target locally verifies the grant without a synchronous cloud call;
3. `target_device_id` and `tenant_id` are cryptographically protected and validated;
4. `ENTERPRISE_ONLY` cannot be bypassed by a controller that skips the Laravel authorization call;
5. replaying one consumed `authorization_id` for another new connection is rejected;
6. denied capabilities cannot be re-enabled by a modified controller;
7. key rotation supports overlapping trusted public keys by `kid`;
8. existing LAN workflows remain available only according to `LAN_ONLY`/`HYBRID` policy;
9. Remote Grant validation does not enter the continuous screen/audio/input/file data path;
10. automated tests demonstrate the required bypass, replay, expiry, tenant, target, and capability protections.
