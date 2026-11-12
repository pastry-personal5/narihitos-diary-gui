# P1-M33: Implementation review and corrections

Status: Archived

Review requested by the owner after the real GPU test. Scope: application image
port/registry, command adapter and PNG decoder boundary, configuration/reload,
authenticated status routes, Krea wrapper, standalone example and live evidence
harness. The owner-approved command interface, dimensions, timeout, seed range,
offline behavior, prompt policy and milestone boundaries remain in effect.

## Findings and fixes

1. **High: interrupted evidence collection could leave GPU work running.** The
   Python harness terminated only the standalone example's group; the production
   supervisor creates a different group for the wrapper and renderer. Telemetry
   errors, Ctrl-C, SIGTERM or the outer timeout could therefore terminate the
   supervisor without stopping inference. The example now watches its control
   stdin and cancels both render tokens on EOF. The harness closes that pipe and
   waits for supervisor cleanup before any emergency termination. Its bounded
   fallback signals observed descendants individually after checking their start
   times, including descendants in separate groups. A sentinel process now has
   RAII cleanup even if the example panics.

2. **Medium: valid pixel decoding was accepted as proof of all PNG structural
   rules.** A CRC-correct PNG containing an ancillary chunk named `abca` (invalid
   reserved bit) was accepted as `Ok((80, 1, 1))` before the fix. ImageIO tolerates
   that form. Validation now rejects invalid chunk names/reserved bits, unknown
   critical chunks, unsupported IHDR color/depth/compression/filter/interlace
   combinations, malformed/duplicate/misplaced palettes and missing indexed
   palettes before system decoding. Legal ancillary chunks and an optional RGBA
   palette still pass; decoded-pixel validation remains mandatory.

3. **Medium: readiness and provenance did not use identical prerequisites.**
   Wrapper adapter globbing included directories, and Python executability was
   not checked. The harness also duplicated the selection algorithm, making its
   recorded selection capable of diverging from the wrapper. Both now use the
   same selection function, regular adapter files and executable Python. The
   wrapper rejects dangling output symlinks before invoking the CLI. The live
   harness verifies selection and all recorded content hashes again after the
   run, failing if they changed.

4. **Low: failed reports omitted the failure cause.** Early preflight failure
   left only a `failed` status and timestamps in the machine-readable report.
   Reports now include exception type/message, including interruptions, while
   retaining the normal build/run logs and GPU samples. These are private
   synthetic-test diagnostics; they do not change API errors or production logs.

## Regression evidence

The new GPU-free Python integration regression builds the real Rust example,
launches a fake engine with a sleeping descendant, proves that the engine's
process group differs from the example's group, and injects a telemetry error.
Closing the control pipe cancels through the real command supervisor. Both owned
processes stop, staging is disposed, and a separately launched sentinel survives.
A separate regression checks retained preflight failure details. No GPU weights
or account are used by these tests.

PNG regressions construct real CRC-correct byte streams around valid compressed
pixels, rather than testing only invalid compressed data. Wrapper regressions
exercise later-sorting directory candidates and dangling output symlinks. API
coverage now explicitly checks that unauthenticated POST check requests fail
without probing.

## Validation and evidence provenance

The original [GPU evidence](milestone-33-gpu-evidence.md) and its artifacts remain
unchanged as historical proof for their recorded source and binary hashes.
The review-run evidence below records the changed source hashes; changed source is not represented as identical to the original captured sources. No dependency, build-tool, diary-format or GUI change was required.

### Reviewed real GPU run

Executed `DIARY_IMAGE_LIVE=1 python3 scripts/smoke/image-generation-live.py` with
real Metal/process access and inference offline. Outcome: PASS. Apple M5 Max,
MLX 0.32.4, mflux 0.23.0, unchanged model/adapter and the same approved dimensions,
steps, scheduler and seed as the original run.

- Render: 111.283 seconds; 1248 × 1664; 1,873,430 encoded bytes; strict structural validation and ImageIO pixel decode passed.
- Cancellation: 10.239 seconds including the ten-second in-flight delay; observed owned PIDs 36956, 36960 and 36991 all exited. Staging was empty and the sentinel survived.
- GPU proof: owned renderer PID 36353 captured executing MLX GPU operations through Metal; 112 accelerator samples retained. Machine-wide utilization reached 100%, with no exclusivity claim.
- Selection and every recorded source/model/adapter hash matched before and after the run.
- Image SHA-256: `650609b1283278adc26f0754407b30002743dde0ac2426041a08b85354b70043`.

Retained artifacts: [report](evidence/review/report.json),
[PNG](evidence/review/krea2-cup.png),
[owned GPU stack excerpt](evidence/review/gpu-stack-excerpt.txt).
Full private logs, all GPU samples and stacks: `tmp/krea GPU evidence nkkojta8/`.
The timing difference is an observation, not an optimization claim or benchmark.

GPU-free validation: five infrastructure image tests and the authenticated image
API test pass; the wrapper contract and opt-in skip pass; all 15 Python harness
regressions pass with process visibility. The PNG test demonstrated the original
invalid-chunk acceptance before the fix and passes afterwards.

Final `scripts/gate.sh`: G01–G07 PASS (`run.R4C6Zo`), with inference stopped
and socket/process visibility enabled. The preceding gate (`run.i9A89b`) passed
all lanes except Clippy, which requested the equivalent standard-library
`is_multiple_of` form for the palette-length check; this lint-only correction
followed the captured GPU run. The final gate validates that final source.
Reviewed wrapper/harness/example source hashes and the retained PNG hash match
the captured provenance. Documentation formatting, local links, completion
checklist and whitespace checks pass. All four findings above are addressed.
