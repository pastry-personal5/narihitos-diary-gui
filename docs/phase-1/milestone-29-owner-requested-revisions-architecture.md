# P1-M29: Owner-requested revisions work plan

Status: Active

Approach and sequencing for the [P1-M29 overview](milestone-29-owner-requested-revisions-overview.md). Filled in per request; this skeleton holds only the working rules.

## Rules

- Group related requests and order them by dependency; do bug fixes before behavior changes that touch the same code.
- Each task names the modules it touches and the test or smoke that proves it. Add a regression test for every bug fix.
- Keep layers and the [architecture](../architecture.md) contracts; an architecture change updates the affected architecture doc in the same change.
- No new dependency, build tool or product behavior outside a request without owner approval.

## Task notes

T21 moves the connection display and manual Connect action from `App.tsx` to a Server card in `SettingsView.tsx`, passing the existing connection state and callback. `useConnection` remains app-scoped. UI tests verify placement, connection states and the disabled action during connection; existing shell tests connect through Settings.

## Risks

_Added per request._
