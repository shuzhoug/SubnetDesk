# SubnetDesk Enterprise Management Design

Date: 2026-08-21

## 1. Goal

Extend SubnetDesk into a centrally managed enterprise remote-access platform for multiple customer organizations, while preserving SubnetDesk's existing LAN/private-network direct-connection model.

The first release must support a single hosted platform serving many tenants, with each tenant able to manage roughly 1,000 to 10,000 devices. The control plane is centralized, while remote desktop media traffic remains peer-to-peer whenever the devices are routable to each other.

## 2. Product Positioning

The enterprise product is a multi-tenant remote device management platform built around the existing SubnetDesk client.

The platform provides:

- tenant and organization management;
- user, department, role, and permission management;
- device enrollment and ownership;
- device groups;
- online/offline status;
- remote-access authorization;
- remote session and administrative audit logs;
- centralized policy delivery;
- enterprise device discovery through the cloud control plane while keeping existing LAN discovery available.

The first phase intentionally excludes billing, payment, ticketing, AI features, cloud drive, ERP functions, asset purchasing, public relay service, and major mobile-control redesign.

## 3. Architectural Principles

### 3.1 Separate control plane and data plane

Laravel and the Device Gateway form the control plane. They decide who a user is, which tenant owns a device, whether a device is online, what policies apply, and whether a remote session is authorized.

Actual screen video, input, clipboard, audio, and file-transfer traffic should continue to use SubnetDesk's existing direct connection path whenever routing permits. Laravel must never proxy remote desktop frames.

### 3.2 Preserve the existing SubnetDesk remote core

Existing mature code for screen capture, input, clipboard, audio/video, file transfer, LAN discovery, and direct TCP connection should remain intact unless an enterprise integration point requires a focused change.

Enterprise functionality should be added as a distinct layer with narrow interfaces into the existing Rust and Flutter code.

### 3.3 Multi-tenant by default

The hosted platform uses one application deployment and one logical database cluster for many organizations.

Business tables that contain tenant-owned data must carry a `tenant_id` and all application queries must be tenant scoped. The first version uses shared tables rather than one database per customer.

### 3.4 Real-time device state does not live in MySQL

MySQL stores durable device metadata and history. Redis stores current online/offline state, current gateway ownership, and short-lived presence metadata.

### 3.5 Enrollment credentials are one-time bootstrap credentials

Employee login, enrollment codes, and preconfigured enterprise installers all converge on a single Device Enrollment API. After enrollment, the client receives its own device credential and no longer uses the original enrollment code.

## 4. High-Level Architecture

```text
                         Browser / Admin UI
                                |
                                v
                    +------------------------+
                    | Laravel Control Plane  |
                    | REST API + Admin UI    |
                    +-----------+------------+
                                |
                 +--------------+--------------+
                 |                             |
                 v                             v
             +-------+                    +---------+
             | MySQL |                    |  Redis  |
             +-------+                    +---------+
                                                ^
                                                |
                                      presence / routing
                                                |
                                      +---------+---------+
                                      |   Device Gateway  |
                                      | WS + HTTPS        |
                                      +---------+---------+
                                                ^
                                                |
                         authenticated persistent connection
                                                |
              +---------------------------------+---------------------------------+
              |                                 |                                 |
              v                                 v                                 v
       SubnetDesk Windows                SubnetDesk macOS                 SubnetDesk Linux
              |                                                                   |
              +------------------- direct remote desktop traffic -----------------+
```

## 5. Component Responsibilities

## 5.1 Laravel Control Plane

Laravel owns durable enterprise business logic.

Responsibilities:

- tenant lifecycle;
- users and authentication;
- departments;
- roles and permissions;
- device metadata;
- device groups;
- enrollment token creation and revocation;
- device credential issuance and revocation;
- policy definitions;
- remote access authorization;
- remote session records;
- audit logs;
- administrative API;
- operator web UI;
- issuing short-lived authorization grants to clients.

Laravel must not receive high-frequency heartbeats from every device under normal operation.

## 5.2 Device Gateway

The Device Gateway is a separate Rust service designed for large numbers of persistent device connections.

Responsibilities:

- accept authenticated WebSocket connections from enrolled clients;
- validate device credentials against cached or signed identity data;
- maintain connection ownership for each online device;
- process heartbeats;
- publish online/offline transitions to Redis;
- receive lightweight device metadata updates;
- deliver policy refresh notifications;
- deliver remote-session authorization notifications where needed;
- expose connection state to the Laravel control plane through Redis and an internal authenticated API or message channel.

The Gateway does not proxy screen/video traffic.

The first deployment may use one Gateway instance. The protocol must allow horizontal scale later by storing gateway ownership in Redis.

## 5.3 SubnetDesk Enterprise Client Layer

The existing SubnetDesk client gains an Enterprise module without replacing the LAN-only features.

Responsibilities:

- cloud account sign-in;
- device enrollment;
- secure persistence of device identity and credential;
- persistent Gateway connection;
- heartbeat and status reporting;
- receiving centralized policies;
- cloud-managed device list;
- remote-access permission check before managed enterprise connections;
- session start/end audit events;
- organization and user display in Flutter;
- continuing to provide existing LAN discovery and manual IP/hostname connection.

## 6. Tenant Model

A tenant represents one customer organization.

Each tenant owns:

- departments;
- users;
- roles;
- devices;
- device groups;
- enrollment tokens;
- policies;
- remote session history;
- audit events.

Cross-tenant access is forbidden by application rules and database query scoping.

A platform administrator may operate across tenants through a separate platform-level permission path. Tenant administrators may access only their own tenant.

## 7. Core Data Model

The following schema names define the first version's logical model. Exact migrations may add framework timestamps, soft deletes, indexes, and foreign-key details as appropriate.

### 7.1 `tenants`

- `id` bigint primary key
- `name` varchar(191)
- `slug` varchar(100) unique
- `status` enum: `active`, `suspended`
- `created_at`
- `updated_at`

### 7.2 `users`

- `id`
- `tenant_id` nullable for platform administrators
- `department_id` nullable
- `name`
- `email`
- `password`
- `status` enum: `active`, `disabled`
- timestamps

Unique email rules may be global or tenant scoped depending on the chosen login UX; the API must resolve a user to exactly one tenant before issuing a tenant session.

### 7.3 `departments`

- `id`
- `tenant_id`
- `parent_id` nullable
- `name`
- timestamps

### 7.4 `roles`

- `id`
- `tenant_id` nullable for system roles
- `name`
- `code`
- timestamps

### 7.5 `permissions`

- `id`
- `code` unique
- `name`

Initial permission codes:

- `tenant.manage`
- `users.manage`
- `departments.manage`
- `devices.view`
- `devices.manage`
- `devices.enroll`
- `devices.remote_control`
- `devices.file_transfer`
- `device_groups.manage`
- `policies.manage`
- `audit.view`

### 7.6 Role mapping tables

Use conventional pivot tables:

- `role_user`
- `permission_role`

Every tenant-owned assignment must be validated against the same tenant.

### 7.7 `devices`

- `id` bigint primary key
- `tenant_id`
- `department_id` nullable
- `owner_user_id` nullable
- `uuid` uuid unique, server-issued immutable device ID
- `device_name`
- `hostname`
- `platform` enum: `windows`, `macos`, `linux`, `android`, `other`
- `os_version` nullable
- `client_version` nullable
- `architecture` nullable
- `lan_address` nullable
- `remote_endpoint` nullable
- `fingerprint` nullable
- `status` enum: `active`, `disabled`, `retired`
- `last_seen_at` nullable
- `enrolled_at`
- timestamps

`last_seen_at` is durable historical information and is not the source of truth for current online status.

### 7.8 `device_groups`

- `id`
- `tenant_id`
- `name`
- `description` nullable
- timestamps

### 7.9 `device_group_members`

- `device_group_id`
- `device_id`
- unique composite index

### 7.10 `enrollment_tokens`

- `id`
- `tenant_id`
- `created_by_user_id`
- `token_hash`
- `label`
- `department_id` nullable
- `device_group_id` nullable
- `expires_at` nullable
- `max_uses` nullable
- `used_count`
- `revoked_at` nullable
- timestamps

Only a cryptographic hash of the token is stored. The plaintext token is shown only when created.

### 7.11 `device_credentials`

- `id`
- `device_id`
- `credential_id` unique
- `secret_hash` or public-key identity material depending on final credential mechanism
- `issued_at`
- `expires_at` nullable
- `revoked_at` nullable
- `last_used_at` nullable

The implementation should prefer asymmetric device credentials if the SubnetDesk key material can be reused cleanly. Otherwise use a high-entropy opaque credential in phase one, stored encrypted in the OS credential store and hashed server-side.

### 7.12 `policies`

- `id`
- `tenant_id`
- `name`
- `version`
- `settings_json`
- timestamps

Initial enterprise policy settings may include:

- allow unattended access;
- allow file transfer;
- allow clipboard;
- LAN listening port;
- permitted source CIDRs;
- Gateway heartbeat interval within server-allowed limits.

### 7.13 `device_policy_assignments`

- `device_id`
- `policy_id`
- timestamps

A later version may support group-level inheritance. Phase one should resolve a single effective device policy server-side.

### 7.14 `remote_sessions`

- `id`
- `tenant_id`
- `requester_user_id`
- `source_device_id` nullable
- `target_device_id`
- `authorization_id` unique
- `started_at` nullable
- `ended_at` nullable
- `result` enum: `authorized`, `denied`, `connected`, `failed`, `completed`
- `denial_reason` nullable
- `source_ip` nullable
- timestamps

No screen contents, clipboard contents, keystrokes, or transferred file contents are stored in this table.

### 7.15 `audit_logs`

- `id`
- `tenant_id` nullable for platform events
- `actor_user_id` nullable
- `actor_device_id` nullable
- `action`
- `target_type`
- `target_id` nullable
- `metadata_json`
- `ip_address` nullable
- `created_at`

Audit records are append-only through application code.

## 8. Redis Presence Model

Redis is the source of truth for current online state.

Recommended keys:

```text
device:presence:{device_uuid}
```

Hash fields:

- `tenant_id`
- `gateway_id`
- `connection_id`
- `connected_at`
- `last_heartbeat_at`
- `client_version`
- `remote_endpoint`

Presence keys use a TTL longer than the expected heartbeat interval. A Gateway refreshes the TTL on every valid heartbeat.

Tenant membership for efficient dashboards:

```text
tenant:{tenant_id}:online_devices
```

Use a Redis Set containing device UUIDs. Gateway disconnect handling removes the device from the set. TTL expiry must also be reconciled by a cleanup process so stale set entries do not remain indefinitely.

Recommended starting heartbeat behavior:

- normal heartbeat interval: 30 seconds;
- presence TTL: 90 seconds;
- after three missed heartbeats, the device is treated as offline.

The server may later tune these values without changing the protocol.

## 9. Device Enrollment

All enrollment paths converge on one server workflow.

## 9.1 Enrollment path A: employee login

1. User signs in to the enterprise account in SubnetDesk.
2. Client requests to enroll the current device.
3. Server checks `devices.enroll` or tenant policy allowing self-enrollment.
4. Server creates the device record.
5. Server issues device identity and Gateway settings.
6. Device establishes its persistent Gateway connection.

## 9.2 Enrollment path B: enterprise enrollment code

1. Tenant administrator creates an enrollment token in Laravel.
2. Client user enters the plaintext code once.
3. Client sends the code plus basic device metadata to the Enrollment API.
4. Server verifies the token hash, tenant status, expiry, revocation state, and use limit.
5. Server creates the device and applies optional department/group defaults.
6. Server increments `used_count` atomically.
7. Server issues device identity.
8. Client erases the enrollment code from memory and never persists it.

## 9.3 Enrollment path C: preconfigured installer

The generated installer or deployment configuration contains:

- `control_plane_url`;
- one-time `enrollment_token`;
- optional department or device-group hint if represented by that token.

The package must not contain a permanent shared tenant secret.

On first launch after installation, the same Enrollment API flow used by path B executes. After successful enrollment, the bootstrap token is deleted locally.

## 9.4 Enrollment API

Initial endpoint:

```text
POST /api/v1/device-enrollments
```

Request fields:

```json
{
  "enrollment_token": "plaintext-one-time-token",
  "hostname": "OFFICE-PC-001",
  "platform": "windows",
  "os_version": "...",
  "client_version": "...",
  "architecture": "x86_64",
  "device_fingerprint": "..."
}
```

Successful response:

```json
{
  "device": {
    "id": "server-device-uuid",
    "tenant_id": 12,
    "name": "OFFICE-PC-001"
  },
  "credential": {
    "credential_id": "...",
    "secret": "returned-once"
  },
  "gateway": {
    "url": "wss://gateway.example.com/device/v1"
  },
  "policy": {
    "version": 1,
    "settings": {}
  }
}
```

The secret is returned only at enrollment or explicit credential rotation.

## 10. Device Authentication and Gateway Protocol

The Gateway protocol uses secure WebSocket over TLS.

Connection URL:

```text
wss://gateway.example.com/device/v1
```

The client presents its `credential_id` and proof of the associated secret during connection authentication. Plain device secrets must never appear in query strings or logs.

After authentication, logical messages are versioned JSON in phase one for implementation speed and observability. A binary protocol may be introduced later without changing the control-plane semantics.

Example client hello:

```json
{
  "type": "hello",
  "protocol_version": 1,
  "device_id": "...",
  "credential_id": "...",
  "client_version": "1.2.3",
  "platform": "windows"
}
```

Example heartbeat:

```json
{
  "type": "heartbeat",
  "sent_at": "2026-08-21T09:00:00Z",
  "lan_addresses": ["192.168.1.20"],
  "remote_endpoint": "10.10.1.20:21118",
  "policy_version": 3
}
```

Example policy refresh notification:

```json
{
  "type": "policy_changed",
  "policy_version": 4
}
```

Clients then fetch the complete effective policy through authenticated HTTPS rather than receiving arbitrary policy payloads over the Gateway connection.

## 11. Remote Access Authorization

Managed enterprise remote access requires platform authorization before the existing direct connection is initiated.

Flow:

```text
Operator selects target device
        |
        v
Client requests authorization from Laravel
        |
        v
Laravel validates tenant, user, role, device state, and policy
        |
   +----+----+
   |         |
 deny      allow
   |         |
 audit       v
          issue short-lived authorization grant
              |
              v
     client starts existing SubnetDesk direct connection
              |
              v
       start/end events are audited
```

Initial endpoint:

```text
POST /api/v1/remote-authorizations
```

Request:

```json
{
  "target_device_id": "...",
  "source_device_id": "...",
  "capabilities": ["remote_control", "clipboard", "file_transfer"]
}
```

Server checks:

1. authenticated user belongs to the target tenant;
2. target device is active;
3. user has `devices.remote_control`;
4. department/device-group policy allows the relationship;
5. requested optional capabilities are allowed;
6. target presence exists in Redis if online access is required.

Successful response:

```json
{
  "authorization_id": "...",
  "expires_at": "2026-08-21T09:01:00Z",
  "target": {
    "device_id": "...",
    "endpoint": "10.10.1.20:21118",
    "fingerprint": "..."
  },
  "capabilities": ["remote_control", "clipboard"]
}
```

Authorization grants should expire quickly; 60 seconds is the initial target.

Phase one does not redesign the complete SubnetDesk transport handshake around the authorization token. Instead, cloud-managed session initiation requires a successful authorization response before invoking the existing connection path, and the target's enterprise policy controls whether managed unattended access is enabled. A later hardening phase may bind the grant cryptographically into the target-side handshake.

## 12. Initial Authorization Rules

The product must support these common cases:

- tenant administrator: may control all active devices in the tenant;
- IT role: may control devices assigned to permitted departments or groups;
- department manager: may control devices in that department when granted permission;
- ordinary employee: may control only owned/assigned devices when tenant policy allows it;
- cross-tenant remote control: always denied.

The permission model combines coarse RBAC permissions with resource scope checks. RBAC alone is not sufficient because a sales manager with remote-control permission must not automatically gain access to finance devices.

## 13. Laravel API Surface for Phase One

All endpoints are under `/api/v1`.

### Authentication

- `POST /auth/login`
- `POST /auth/logout`
- `GET /me`

### Tenants

- `GET /tenant`
- `PATCH /tenant`

Platform-administrator tenant provisioning may use separate `/api/platform/v1` endpoints and is not exposed to tenant users.

### Departments

- `GET /departments`
- `POST /departments`
- `PATCH /departments/{department}`
- `DELETE /departments/{department}`

### Users

- `GET /users`
- `POST /users`
- `GET /users/{user}`
- `PATCH /users/{user}`
- `POST /users/{user}/roles`

### Devices

- `GET /devices`
- `GET /devices/{device}`
- `PATCH /devices/{device}`
- `POST /devices/{device}/disable`
- `POST /devices/{device}/credential-rotation`
- `GET /devices/{device}/policy`

### Enrollment

- `GET /enrollment-tokens`
- `POST /enrollment-tokens`
- `DELETE /enrollment-tokens/{token}`
- `POST /device-enrollments`

### Device groups

- `GET /device-groups`
- `POST /device-groups`
- `PATCH /device-groups/{group}`
- `DELETE /device-groups/{group}`
- `POST /device-groups/{group}/devices`
- `DELETE /device-groups/{group}/devices/{device}`

### Policies

- `GET /policies`
- `POST /policies`
- `PATCH /policies/{policy}`
- `POST /devices/{device}/policy`

### Remote access

- `POST /remote-authorizations`
- `POST /remote-sessions/{authorization}/started`
- `POST /remote-sessions/{authorization}/ended`

### Audit

- `GET /audit-logs`
- `GET /remote-sessions`

## 14. Client Code Organization

Enterprise code should be isolated from the existing remote-control engine.

Recommended Flutter additions under `flutter/lib/enterprise/`:

```text
flutter/lib/enterprise/
  api/
    enterprise_api.dart
  auth/
    account_session.dart
  devices/
    enterprise_device.dart
    device_repository.dart
  enrollment/
    enrollment_service.dart
  gateway/
    gateway_state.dart
  policy/
    enterprise_policy.dart
  ui/
    enterprise_home_page.dart
    enterprise_devices_page.dart
    enrollment_page.dart
```

Recommended Rust additions under a focused module such as:

```text
src/enterprise/
  mod.rs
  identity.rs
  gateway.rs
  heartbeat.rs
  policy.rs
  secure_store.rs
```

Exact placement should follow compilation boundaries already used by the repository, but enterprise code should not be scattered through unrelated screen capture or input modules.

## 15. Client Local State

Persist only the information required after enrollment:

- control plane URL;
- tenant ID;
- device ID;
- credential ID;
- device credential secret/private key;
- Gateway URL;
- last accepted policy version;
- optional user login refresh token if account login is enabled on that device.

Secrets must use the platform credential store where practical:

- Windows Credential Manager / DPAPI-backed storage;
- macOS Keychain;
- Linux Secret Service when available, with a documented secure fallback if unavailable.

Enrollment codes must never be stored after successful enrollment.

## 16. Flutter Enterprise UX

The desktop home experience keeps both enterprise and LAN workflows.

Primary sections:

- My Devices;
- All Devices;
- Favorites;
- Recent Connections;
- Device Groups;
- Nearby LAN Devices.

Device rows show at minimum:

- display name;
- hostname;
- platform;
- online/offline state;
- assigned department/group;
- last seen when offline.

Selecting an enterprise-managed device presents actions allowed by policy, such as Remote Control, File Transfer, and Device Information.

LAN-discovered devices continue to use the existing direct workflow and are visually separated from enterprise-managed devices.

## 17. Presence and Scale Expectations

The target is at least 10,000 online devices for one tenant and multiple tenants on the same hosted platform.

The design avoids high-frequency writes to MySQL by keeping presence in Redis and sending heartbeats only to the Gateway.

At a 30-second interval, 10,000 devices produce roughly 333 heartbeats per second for one fully online tenant. This is a reasonable starting load for a lightweight asynchronous Rust Gateway when heartbeats do not cause synchronous MySQL work.

Horizontal Gateway scaling must use a stable `gateway_id` per instance and Redis presence records so the control plane can determine which Gateway owns a device connection.

## 18. Failure Handling

### Gateway unavailable

- client retries with exponential backoff and jitter;
- existing LAN direct access remains usable if local policy permits it;
- device appears offline in enterprise cloud status after presence TTL expires.

### Redis unavailable

- Gateways keep current WebSocket connections alive when possible;
- presence updates are retried;
- Laravel reports real-time status as degraded rather than treating Redis failure as proof that all devices are offline.

### Laravel unavailable

- existing connected remote sessions should not be forcibly terminated solely because the control plane is briefly unavailable;
- new enterprise-managed sessions cannot obtain new authorization until the API recovers;
- Gateway device connections may remain established.

### Device credential revoked

- new Gateway authentication is rejected;
- if revocation is propagated to the owning Gateway, the active device connection is closed;
- device must be re-enrolled or receive a replacement credential through an approved administrative flow.

## 19. Security Requirements

- TLS is mandatory for all public control-plane and Gateway traffic.
- Device enrollment tokens use at least 128 bits of cryptographic randomness.
- Plain enrollment tokens and device secrets are never logged.
- Tenant ownership is validated server-side for every tenant-owned resource operation.
- Device credentials are revocable independently per device.
- Remote authorization grants are short lived.
- Audit logs record security-sensitive administrative actions and remote session lifecycle events.
- Existing SubnetDesk device fingerprint verification remains enabled where applicable.
- CIDR restrictions remain available as device policy controls.
- The platform does not collect screen contents, clipboard contents, typed keys, or file payloads as audit data.

## 20. Audit Events

At minimum, phase one records:

- tenant settings changed;
- user created/disabled;
- role assignment changed;
- enrollment token created/revoked;
- device enrolled;
- device disabled;
- device credential rotated/revoked;
- device group changed;
- policy changed/assigned;
- remote authorization granted/denied;
- remote session started;
- remote session ended.

Each audit event includes actor, tenant, action, target identifiers, timestamp, and relevant non-secret metadata.

## 21. Deployment Topology

Initial production topology:

```text
Internet / Private WAN
        |
  Reverse Proxy / TLS
        |
   +----+--------------------+
   |                         |
Laravel API/Web        Device Gateway
   |                         |
   +------------+------------+
                |
        +-------+-------+
        |               |
      MySQL           Redis
```

Laravel queue workers run separately from HTTP workers. Redis may serve queues and presence initially, but presence keys should use a dedicated key namespace and can later move to a dedicated Redis cluster.

## 22. Repository Strategy

The existing `shuzhoug/SubnetDesk` repository remains the client repository.

Recommended project separation:

- `SubnetDesk`: existing Rust + Flutter client with the enterprise client layer;
- a separate Laravel repository for the control plane;
- a separate Rust repository or workspace for Device Gateway.

Keeping the Laravel application out of the large RustDesk-derived repository reduces build coupling and makes deployment ownership clearer.

Shared protocol definitions should be versioned explicitly. Phase one may document JSON message schemas in both repositories; a later version can introduce generated shared schema artifacts if duplication becomes a maintenance problem.

## 23. Phase One Delivery Sequence

The first phase should be implemented as independently testable vertical slices rather than building the entire backend before touching the client.

Recommended order:

1. tenant, user, permission, and device schema foundation;
2. enrollment token creation and Device Enrollment API;
3. client secure device identity persistence;
4. Gateway authentication, heartbeat, and Redis presence;
5. Laravel device list with online/offline status;
6. Flutter enterprise device list;
7. remote authorization API and resource-scope rules;
8. client authorization-before-connect integration;
9. remote session lifecycle audit;
10. device groups and basic policy delivery;
11. preconfigured installer bootstrap support;
12. hardening, load testing, deployment documentation.

## 24. Acceptance Criteria for Phase One

Phase one is complete when all of the following are demonstrably true:

1. One hosted deployment supports multiple tenants with data isolation.
2. A tenant administrator can create departments, users, roles, enrollment codes, groups, and policies.
3. A device can enroll by employee login, enrollment code, or preconfigured installer bootstrap.
4. Enrollment bootstrap credentials are not reused as permanent device credentials.
5. Enrolled clients maintain authenticated Gateway connections.
6. Laravel shows current device online/offline state backed by Redis presence.
7. A user can see enterprise devices permitted by tenant/resource scope.
8. A managed remote connection requires a successful authorization check before the existing SubnetDesk connection is started.
9. Cross-tenant remote access is denied.
10. Existing LAN discovery and direct manual connection continue to work.
11. Remote desktop video/audio/input traffic does not pass through Laravel or the Device Gateway.
12. Remote session start/end events and key administrative changes appear in audit history.
13. The platform can sustain a representative load test equivalent to 10,000 connected devices sending 30-second heartbeats without synchronous MySQL writes per heartbeat.

## 25. Deferred Work

Explicitly deferred until after phase one:

- public NAT traversal, rendezvous, or relay infrastructure;
- subscription billing and payment;
- customer self-service plans;
- support ticketing;
- AI assistance;
- cloud file storage;
- ERP/CRM features;
- advanced asset inventory and software inventory;
- remote command shell;
- mass software deployment;
- session recording;
- mobile-first redesign;
- cryptographic binding of the cloud authorization grant into the existing SubnetDesk target-side transport handshake.

These items require separate designs before implementation.
