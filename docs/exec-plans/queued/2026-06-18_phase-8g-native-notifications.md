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

- [ ] Define notification message, capability, permission, and delivery outcome.
- [ ] Add notification port and Tauri adapter.
- [ ] Implement durable threshold claim/status behavior.
- [ ] Integrate settings and runtime capability checks.
- [ ] Deliver evaluation decisions after commit.
- [ ] Prove duplicate prevention, suppression, failure, and safe retry.
- [ ] Add supported-platform manual runtime evidence and document limitations.

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
- Exact crash-recovery semantics between claim and delivery must be decided and
  recorded before implementation, preserving at-most-once delivered behavior.

## Verification

- Command: `pnpm verify`
- Outcome: not run yet

## Runtime Evidence

- Not run yet.

## Follow-Up Debt

- Notification action buttons and notification center history are not required.
