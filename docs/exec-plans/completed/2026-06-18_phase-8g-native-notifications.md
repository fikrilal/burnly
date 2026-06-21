# 2026-06-18 Phase 8G Native Notifications

## Objective

Deliver eligible budget threshold notifications through a narrow native
notification port with durable duplicate prevention and explicit capability and
permission state.

## Acceptance Criteria

- Notification delivery is disabled unless settings and a budget enable it.
- Eligibility is decided by the budget application layer, not the Tauri adapter.
- One threshold is not delivered twice for the same budget, period, and
  aggregation timezone.
- Delivery status records delivered, failed, or suppressed outcomes.
- Failure does not roll back usage reconciliation or budget progress.
- Permission/capability state is visible and unsupported platforms do not report
  success.
- Retry policy cannot duplicate a previously delivered notification.

## Risk Class

`high`

This phase combines native side effects, user permission, durable idempotency,
and post-commit failure handling.

## Impact Areas

- Notification port and application delivery service
- SQLite notification-state repository
- Tauri native notification adapter and capability reporting
- Settings notification controls
- Runtime evidence and platform tests

## Design Review

- What complexity is being introduced? Permission/capability handling, durable
  claim identity, delivery, and failure recording.
- Which decisions are hidden inside the owning module? Application owns
  eligibility/idempotency; the adapter owns OS delivery mechanics.
- Is each new interface simpler than its implementation? The port receives a
  notification message and returns a typed delivery outcome.
- What special cases exist, and can the design eliminate them? Suppressed,
  failed, and delivered are explicit states; retry is policy over failed state.
- Why is each new abstraction needed now? Native delivery must remain outside
  budget rules and SQLite transactions.
- Can an existing module absorb this responsibility cleanly? Platform provides
  the adapter; budgets coordinate decisions without knowing Tauri.

## Checklist

- [x] Define notification message, capability, permission, and delivery outcome.
- [x] Add notification port and Tauri adapter.
- [x] Implement durable threshold claim/status behavior.
- [x] Integrate settings and runtime capability checks.
- [x] Deliver evaluation decisions after commit.
- [x] Prove duplicate prevention, suppression, failure, and safe retry.
- [x] Add supported-platform manual runtime evidence and document limitations.

## Test Plan

- Behavior and invariants to prove: one delivery per identity; disabled or
  unsupported delivery is suppressed; failed delivery preserves committed data.
- Lowest stable test layer: application tests with a small fake port and real
  SQLite state tests.
- Failure paths: denied permission, unsupported platform, adapter failure,
  process restart between claim and delivery.
- Fixtures or fakes: recording/failing notification port and fixed clock.
- Runtime or platform evidence: real native notification on each claimed
  platform where available.
- Relevant commands: focused Rust tests, `pnpm verify`,
  `pnpm verify:runtime`.

## Decisions

- Do not hold a SQLite transaction open while invoking the OS notification API.
- Claim a threshold identity durably before invoking the OS. The initial claim
  uses `failed` as the conservative persisted state because the locked schema
  has no in-flight status.
- Existing claims are never delivered automatically, including `failed`
  claims. A crash between claim and delivery can therefore lose one alert, but
  cannot create a duplicate alert after restart.
- Definite adapter failures are recorded as `failed`; a future explicit retry
  policy requires a schema extension that distinguishes safe failures from
  ambiguous interrupted attempts.

## Verification

- Command: `pnpm verify`
- Outcome: passed. Frontend tests passed, Rust tests passed with the opt-in
  notification smoke test ignored by default, clippy and rustfmt passed, and
  architecture, contract, migration, collector, and duplication gates passed.
- Command: `pnpm verify:runtime`
- Outcome: passed on Ubuntu GNOME/X11, including Tauri prerequisites, frontend
  production build, IPC bridge tests, platform tests, scheduler tests, and 18
  Playwright desktop evidence tests.
- Command:
  `cargo test --manifest-path src-tauri/Cargo.toml platform::notifications::tests::smoke_sends_a_native_notification -- --ignored --nocapture`
- Outcome: passed; the native adapter reported granted permission and the
  operating-system notification API accepted the Burnly smoke notification.

## Runtime Evidence

- Tested on Linux x64, Ubuntu GNOME, X11.
- The Rust `tauri-plugin-notification` integration was detected and a real
  notification smoke delivery returned success.
- macOS and Windows notification presentation and permission behavior remain
  unverified in this environment.

## Follow-Up Debt

- Notification action buttons and notification center history are not required.
- Automatic retry of failed or interrupted notification attempts is deliberately
  absent. Supporting it safely requires persisted attempt certainty rather than
  reusing the current three-state schema.
