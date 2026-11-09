# P1-M32: Settings and status line indicators

Status: Active

Owner request: put Server first and Keyboard Shortcuts last in Settings, with
Status Line immediately before Keyboard Shortcuts. Provide independent saved
visibility toggles for Generate ready, Review ready and Chat ready. Ready labels
reflect current action eligibility; hiding them does not hide operational status.

Use the existing guarded client user-settings store with a validated `status_line`
section, defaulting all three indicators to visible. Reuse application/controller
eligibility for generation and paragraph review; chat readiness follows connection,
profile, locale and active-run guards.

- [x] Reorder Settings and provide accessible indicator toggles.
- [x] Persist visibility with defaults, validation and preservation of other sections.
- [x] Render each enabled indicator only when its action is ready.
- [x] Pass focused regressions and all seven gate lanes.

Evidence: delegated runner passed 42 focused settings/profile/client tests, including
Settings ordering, independent persisted toggles and footer visibility, and all
seven gate lanes. The 92 native client tests passed with localhost socket
permission after sandbox restrictions blocked loopback setup. Native visual
inspection was not performed.

## Color styling follow-up

- [x] Use foreground colors only: green Generate, violet Review and cyan Chat readiness text.
- [x] Color AI activity by state, preserving text labels and the visibility preferences.
- [x] Pass the repository gate and existing status-line regressions.

Color evidence: delegated validation passed all status/settings regressions and
seven gate lanes (Rust loopback tests with socket permission). Each foreground
color exceeds 4.5:1 contrast against the existing #1f1f1f background.

## Remove autosave message follow-up

- [x] Remove the static autosave message from the status line and update affected references.
- [x] Pass the existing status/settings regressions and repository gate.

Removal evidence: delegated validation passed 23 existing status/settings tests
and all seven gate lanes, with localhost permission for Rust loopback coverage.

## Left alignment follow-up

- [x] Lay out status line items from the left with consistent spacing and wrapping.
- [x] Pass the repository gate.

Alignment evidence: delegated runner passed all seven gate lanes, including
92 native Rust tests with localhost socket permission.
