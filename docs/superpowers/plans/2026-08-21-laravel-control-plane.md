# Laravel Control Plane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the multi-tenant Laravel API and admin control plane for enterprise users, devices, enrollment, policies, authorization, and audit.

**Architecture:** Laravel owns durable tenant-scoped business state in MySQL and projects live device state from Redis. Device enrollment exchanges one-time bootstrap credentials for permanent per-device credentials. Remote authorization returns short-lived grants but never carries remote desktop media.

**Tech Stack:** PHP 8.3+, Laravel 12+, MySQL 8, Redis 7, Pest/PHPUnit, Laravel Sanctum, queue workers.

**Spec:** `docs/superpowers/specs/2026-08-21-enterprise-management-design.md`

## Global Constraints

- Shared database with tenant-scoped business tables.
- Cross-tenant access must fail server-side even if a client submits foreign IDs.
- Device heartbeats do not write synchronously to MySQL.
- Plain enrollment tokens and device secrets are never stored or logged.
- Remote authorization grants initially expire after 60 seconds.
- TDD is mandatory.

---

### Task 1: Bootstrap Control Plane and Tenant Context

**Files:**
- Create: `app/Models/Tenant.php`
- Create: `app/Support/Tenancy/TenantContext.php`
- Create: `app/Http/Middleware/ResolveTenant.php`
- Create: `database/migrations/*_create_tenants_table.php`
- Modify: `bootstrap/app.php`
- Test: `tests/Feature/Tenancy/TenantContextTest.php`

**Interfaces:**
- Produces: `TenantContext::id(): ?int`, `TenantContext::requireId(): int`

- [ ] **Step 1: Write failing tenant-context tests** covering authenticated tenant resolution and rejection when no tenant is available.
- [ ] **Step 2: Run** `php artisan test tests/Feature/Tenancy/TenantContextTest.php` and verify failure.
- [ ] **Step 3: Implement** `tenants`, `Tenant`, `TenantContext`, and `ResolveTenant` with no implicit fallback to another tenant.
- [ ] **Step 4: Re-run test** and verify pass.
- [ ] **Step 5: Commit** `feat: add tenant context foundation`.

### Task 2: Users, Departments, Roles, and Permissions

**Files:**
- Create: `app/Models/Department.php`
- Create: `app/Models/Role.php`
- Create: `app/Models/Permission.php`
- Create migrations for `departments`, `roles`, `permissions`, `role_user`, `permission_role`
- Modify: `app/Models/User.php`
- Create: `database/seeders/PermissionSeeder.php`
- Test: `tests/Feature/Tenancy/CrossTenantIdentityTest.php`

**Interfaces:**
- Produces: `User::hasPermission(string $code): bool`

- [ ] Write tests proving a user in tenant A cannot attach tenant B departments or roles.
- [ ] Run tests and verify RED.
- [ ] Implement tenant-aware relationships and permission resolution.
- [ ] Seed the initial permission codes from the design spec.
- [ ] Run tests and verify GREEN.
- [ ] Commit `feat: add enterprise identity and permissions`.

### Task 3: Devices and Device Groups

**Files:**
- Create: `app/Models/Device.php`
- Create: `app/Models/DeviceGroup.php`
- Create migrations for `devices`, `device_groups`, `device_group_members`
- Create: `app/Http/Controllers/Api/V1/DeviceController.php`
- Create: `app/Http/Controllers/Api/V1/DeviceGroupController.php`
- Modify: `routes/api.php`
- Test: `tests/Feature/Api/Devices/DeviceIsolationTest.php`

**Interfaces:**
- Produces: `GET /api/v1/devices`, `GET/PATCH /api/v1/devices/{device}`
- Produces group CRUD and membership endpoints.

- [ ] Write a test creating devices in two tenants and prove foreign device IDs return 404/403 without leaking metadata.
- [ ] Implement migrations/models/scopes/controllers.
- [ ] Add indexes on `tenant_id`, `uuid`, `department_id`, and common status filters.
- [ ] Verify tests pass.
- [ ] Commit `feat: add tenant scoped device management`.

### Task 4: Enrollment Tokens

**Files:**
- Create: `app/Models/EnrollmentToken.php`
- Create: `app/Services/Enrollment/CreateEnrollmentToken.php`
- Create: `app/Http/Controllers/Api/V1/EnrollmentTokenController.php`
- Create migration for `enrollment_tokens`
- Test: `tests/Feature/Api/Enrollment/EnrollmentTokenTest.php`

**Interfaces:**
- Produces: plaintext token only from creation response.
- Stores: `token_hash` only.

- [ ] Write tests verifying plaintext token is returned once and database never contains it.
- [ ] Write tests for expiry, revocation, and `max_uses`.
- [ ] Implement random token generation using at least 128 bits entropy and keyed/hash comparison resistant to timing leakage.
- [ ] Verify GREEN.
- [ ] Commit `feat: add one-time enrollment tokens`.

### Task 5: Device Enrollment API

**Files:**
- Create: `app/Models/DeviceCredential.php`
- Create migration for `device_credentials`
- Create: `app/Services/Enrollment/EnrollDevice.php`
- Create: `app/Http/Controllers/Api/V1/DeviceEnrollmentController.php`
- Test: `tests/Feature/Api/Enrollment/DeviceEnrollmentTest.php`

**Interfaces:**
- Consumes: valid enrollment token.
- Produces: `POST /api/v1/device-enrollments` response containing permanent device ID, credential ID, credential secret once, Gateway URL, and effective policy.

- [ ] Write failing happy-path test.
- [ ] Write failing tests for expired, revoked, exhausted, and cross-tenant enrollment token misuse.
- [ ] Implement enrollment in one DB transaction with atomic `used_count` update.
- [ ] Hash permanent device secret server-side and ensure response secret is not logged.
- [ ] Verify tests pass.
- [ ] Commit `feat: add device enrollment exchange`.

### Task 6: Policies

**Files:**
- Create: `app/Models/Policy.php`
- Create: `app/Models/DevicePolicyAssignment.php`
- Create migrations for `policies`, `device_policy_assignments`
- Create: `app/Services/Policy/ResolveEffectivePolicy.php`
- Create policy controllers/routes.
- Test: `tests/Feature/Api/Policies/EffectivePolicyTest.php`

**Interfaces:**
- Produces: `ResolveEffectivePolicy::forDevice(Device $device): array`
- Produces: `GET /api/v1/devices/{device}/policy`

- [ ] Test default policy, explicit device assignment, and cross-tenant assignment rejection.
- [ ] Implement one effective policy per device for phase one.
- [ ] Verify version increments on settings changes.
- [ ] Commit `feat: add enterprise device policies`.

### Task 7: Redis Presence Projection

**Files:**
- Create: `app/Services/Presence/DevicePresenceRepository.php`
- Create: `app/Data/DevicePresence.php`
- Modify device serialization/controller responses.
- Test: `tests/Feature/Api/Devices/DevicePresenceTest.php`

**Interfaces:**
- Consumes Redis hash `device:presence:{uuid}`.
- Produces: nullable `DevicePresence` with `online`, gateway, endpoint, timestamps.

- [ ] Write tests using Redis/fake repository proving MySQL `last_seen_at` does not determine current online state.
- [ ] Implement repository with graceful degraded behavior if Redis is unavailable.
- [ ] Add `online`, `last_heartbeat_at`, and endpoint fields to device API projection.
- [ ] Commit `feat: project realtime device presence`.

### Task 8: Remote Authorization

**Files:**
- Create: `app/Models/RemoteSession.php`
- Create migration for `remote_sessions`
- Create: `app/Services/RemoteAccess/AuthorizeRemoteAccess.php`
- Create: `app/Http/Controllers/Api/V1/RemoteAuthorizationController.php`
- Test: `tests/Feature/Api/RemoteAccess/RemoteAuthorizationTest.php`

**Interfaces:**
- Produces: `POST /api/v1/remote-authorizations`
- Returns: `authorization_id`, `expires_at`, target endpoint/fingerprint, allowed capabilities.

- [ ] Write test for admin allowed within own tenant.
- [ ] Write tests for cross-tenant denial, disabled target, missing permission, out-of-scope department/group, unavailable target presence.
- [ ] Implement resource-scope authorization separately from coarse RBAC.
- [ ] Store grant/denial record without remote media or sensitive content.
- [ ] Verify 60-second expiry.
- [ ] Commit `feat: authorize managed remote sessions`.

### Task 9: Audit and Session Lifecycle

**Files:**
- Create: `app/Models/AuditLog.php`
- Create migration for `audit_logs`
- Create: `app/Services/Audit/AuditLogger.php`
- Create session started/ended endpoints.
- Create audit query controller.
- Test: `tests/Feature/Api/Audit/AuditLogTest.php`

**Interfaces:**
- Produces append-only `AuditLogger::record(...)`.
- Produces `/remote-sessions/{authorization}/started` and `/ended`.

- [ ] Test append-only administrative events.
- [ ] Test only the authorized tenant can mark or view its sessions.
- [ ] Implement audit events listed in the spec.
- [ ] Verify clipboard contents, keystrokes, screen data, and file payloads are never accepted as audit fields.
- [ ] Commit `feat: add enterprise audit trail`.

### Task 10: Control Plane CI and Security Regression Suite

**Files:**
- Create: `.github/workflows/test.yml`
- Create: `tests/Feature/Security/TenantBoundaryTest.php`
- Create: `tests/Feature/Security/SecretLeakageTest.php`
- Modify: `phpunit.xml`

- [ ] Add a consolidated test proving two tenants cannot read/update/enroll/authorize across tenant boundaries.
- [ ] Add log assertions proving enrollment token and device credential plaintext are absent.
- [ ] Run full `php artisan test`.
- [ ] Run static analysis/lint configured for the project.
- [ ] Commit `test: harden enterprise control plane boundaries`.

## Plan Acceptance

The Laravel plan is complete when the API can execute enrollment and authorization flows for multiple tenants, device status is read from Redis, cross-tenant tests are green, and no remote screen/audio/input traffic endpoint exists in this application.