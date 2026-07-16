# Upload Policy

## Status

Accepted product behavior.

This document defines when Burnly uploads local usage and which local data is
included. Engineering proposals and API contracts must implement this behavior
without redefining it.

## Product Goal

Make web history useful without weakening Burnly's local-first behavior.

- Burnly works locally without an account or network.
- Creating an account accepts upload of the allowed aggregate data.
- A signed-in desktop uploads automatically; signing out stops new uploads.
- Local refresh and local usage remain correct when upload fails.

## Consent And Session Policy

Account registration on Burnly Web owns privacy-policy and terms acceptance.
Desktop does not show a second upload toggle or consent checkbox.

| Desktop state              | Upload behavior                                          |
| -------------------------- | -------------------------------------------------------- |
| Signed out                 | No collect API calls; usage stays local                  |
| Signed in                  | Upload allowed aggregate data automatically              |
| Sign out                   | Stop new upload requests; keep local data and device id  |
| Different account signs in | Never reuse another account's baseline or pending upload |

## Allowed Data

Upload only:

- daily token totals,
- daily model breakdowns,
- estimated cost metadata when available,
- source and model identifiers,
- reporting timezone,
- device name, platform, and app version.

Never upload:

- project paths or path fingerprints,
- session identifiers or session rows,
- collector payloads or diagnostics,
- prompts, responses, code, or files,
- credentials or access/refresh tokens.

## Upload Scope

### First Upload

The first upload for each account and desktop installation sends all available
local daily history. This applies even when Burnly already completed its local
baseline before the user signed in.

Large history may be split into smaller requests. The first cloud baseline is
complete only after every request is accepted.

### Later Uploads

Later uploads follow the successful daily scope produced by
`refresh-policy.md`.

| Local refresh                                                  | Upload                                                |
| -------------------------------------------------------------- | ----------------------------------------------------- |
| Full refresh or explicit resync                                | Daily history for the successfully refreshed targets  |
| Scheduled, resume, startup-after-gap, or normal manual refresh | Same catch-up date range and successful daily targets |
| Tray freshness refresh                                         | Today for the successful daily targets                |
| Partial refresh                                                | Scope of successful daily targets only                |
| No committed daily facts                                       | No new upload                                         |

A failed collector must not prevent successfully refreshed collectors from
uploading.

## Trigger Policy

```text
Sign in without a cloud baseline       -> upload all local daily history
Refresh commits eligible daily facts   -> upload that refresh scope
Startup with a pending upload          -> retry it for the same account
Manual upload retry                    -> retry the saved request; do not refresh
Sign out                               -> stop new upload requests
```

## Failure Policy

- Save an upload before sending it so app or network failure can be retried.
- Retry the same data without creating a duplicate server write.
- New local refreshes must not change a request whose server result is unknown.
- Repeated offline refreshes may be combined into one later upload scope.
- Upload failure must not fail or roll back local refresh.

## Desktop Surface

Settings may show upload progress, last success, a safe error, and Retry under
the signed-in account. Burnly does not provide a separate upload enable/disable
control in v1.

## Non-Goals

- Cloud-to-desktop history download.
- Session or project upload.
- Per-metric upload controls.
- Public profile or leaderboard behavior.
- Cloud retention and account-deletion rules; backend product policy owns them.
