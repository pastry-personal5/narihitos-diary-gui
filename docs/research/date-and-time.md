# Date and time findings

Status: Active

Research date: 2026-11-05. Covers P1-M1 R8.1–R8.3 and R1.4. Source-format evidence is in [source formats and prior art](source-formats-and-prior-art.md); the algorithms below are recommendations for P1-M2, with the import-zone/saved-date policy confirmed by the owner.

## Distinguish date from instant

The selected diary day is a civil date. Snippet creation, snippet modification, import, and Entry save are distinct instants. An Entry's diary date is not necessarily the save instant's local date: late writing and the D-1/D/D+1 selection are explicitly intended.

Recommend storing the Entry's explicit diary date and timezone identifier separately from UTC creation/update instants. An offset can preserve the clock interpretation of a single instant, but cannot describe future/historical timezone transitions. Compare storing an IANA zone plus the relevant numeric offset with offset-only metadata; do not infer a geographic zone from Ayumi's `+09:00`. Preserve the source's serialized timestamp as provenance.

Owner decision (2026-11-05): capture the system timezone when importing and preserve saved Entry dates. A configured diary zone was the alternative considered and was not selected. Capture one zone for the whole import operation, not a changing zone per file. Preserve the original source offset as provenance; it does not override the operation zone.

## Import bounds

For selected date D and chosen zone Z, form the three civil dates D-1, D, D+1. Determine the zone-aware start of D-1 and start of D+2, convert them to UTC, and scan every UTC date directory intersecting that half-open interval. Then parse each snippet's authoritative instant, convert it to Z, and retain only snippets whose local date belongs to the selected three-date set. Directory discovery is a coarse filter; timestamps determine inclusion.

Do not scan only the three UTC folders bearing the local date names. For positive offsets the first local day can begin in the previous UTC date; negative offsets can extend the last day into the next UTC date. Do not compute a three-day window as 72 elapsed hours or silently use the current offset for old snippets. Historical transitions can skip/repeat midnight or a whole date; use a documented start-of-day/disambiguation policy and report an invalid selected civil day rather than assuming every midnight exists.

## R8.2 Library comparison

| Option | Evidence | Tradeoff for this app |
|---|---|---|
| Chrono 0.4.45 + Chrono-TZ 0.10.4 | [Chrono](https://docs.rs/chrono/0.4.45/chrono/) separates naive dates and timezone-aware values; [Chrono-TZ](https://docs.rs/chrono-tz/0.10.4/chrono_tz/) supplies named-zone rules | Familiar explicit types; named-zone database and updates must be managed alongside Chrono. Conversion can return unique, ambiguous, or missing local times. |
| Jiff 0.2.39 | [Jiff docs](https://docs.rs/jiff/0.2.39/jiff/) integrate timestamps, civil values, spans, and named zones | Fits calendar arithmetic directly; defaults resolve some ambiguity automatically, so app policy must still be explicit. Check timezone-database feature behavior and binary footprint in P1-M2. |
| TypeScript `Intl.DateTimeFormat` | [ECMA-402 date formatting](https://tc39.es/ecma402/#datetimeformat-objects) | Suitable for display with an explicit zone, not the sole owner of timezone interval calculations. |
| JavaScript Temporal | [Temporal specification](https://tc39.es/proposal-temporal/) defines separate plain dates, instants, and zoned datetimes | Availability in the target WKWebView is not established by a Node version or a browser preview. Feature-detect and test the actual minimum supported macOS before relying on it; a polyfill would be a new dependency requiring approval. |

Recommendation: perform canonical selection/date conversion in Rust and send ISO civil dates/instants plus the operation zone through Tauri. Keep frontend calendar navigation civil-date based. P1-M2 selects the Rust library after checking license/features and missing-time behavior; no dependency is adopted here. [Chrono's `LocalResult`](https://docs.rs/chrono/0.4.45/chrono/offset/enum.LocalResult.html) explicitly represents gaps/folds. Jiff offers explicit disambiguation in addition to its defaults. Neither library removes the need for product rules.

## Reference checks and proposed implementation fixtures

Ran an in-memory Python `datetime`/`zoneinfo` check using synthetic dates; start/end were constructed as civil midnights, then converted to UTC before subtracting. Expected and observed:

| Zone/day | UTC start → UTC end | Elapsed hours |
|---|---|---|
| New York, 2026-03-08 | 05:00Z → next day 04:00Z | 23 |
| New York, 2026-11-01 | 04:00Z → next day 05:00Z | 25 |
| Seoul, 2026-01-01 | prior year 12-31 15:00Z → 01-01 15:00Z | 24 |

All three expected durations matched. This validates the reasoning against the host Python timezone data only, not an unimplemented Rust module or WKWebView. Add Rust fixtures for these cases, exact half-open boundaries, leap days, non-hour offsets, repeated local times, skipped midnight/date, malformed instants, changed system zone, and an Entry saved days after its diary date. Compare the full three-date inclusion set, not just interval length.

P1-M2 implication: the import API needs both a civil date and a chosen timezone, and persisted Entry metadata needs an explicit date/zone contract. The owner settled import-zone capture and stable saved dates; authoritative timestamp conflict handling and ambiguous/skipped-date handling remain to be specified.
