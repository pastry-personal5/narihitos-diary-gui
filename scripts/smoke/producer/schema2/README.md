# Mobile schema 2 producer codec fixture

Status: Active

Read-only source snapshot from `narihitos-diary-mobile` on 2026-11-09. Initially inspected in a working tree based on `6f62fd1c68f9bf37d27709d65bfe66aecb1771d5`; all three captured files were subsequently verified byte-for-byte against producer commit `f6cf0dc6c382aecef66b50d41dc7f05faaf0f411`, which includes manual-snippet support. Files are copied byte-for-byte from `NarihitosDiary/Domain` and `NarihitosDiary/Adapters`, under the accompanying Apache-2.0 license. These fixtures are test inputs, not runtime dependencies.

| File                              | SHA-256                                                            |
| --------------------------------- | ------------------------------------------------------------------ |
| `DiarySnippet.swift`              | `debf86bdd26c590ab3aa6735f28fd8c8bf4b35d138d819d5b6b918282b795a6b` |
| `DiarySnippetLayout.swift`        | `fb5f4a3d2ce9aecac7414e827d6ef5cff40ea945a33b3066aca26cc06d0c23d4` |
| `DiarySnippetMarkdownCodec.swift` | `4f51b2756af13849ab0de68cef6f8bd7d16fac8437d2e2c6ab4db9f63233b169` |

Run `python3 scripts/smoke/producer.py --schema2` to compile these standalone files with the existing harness and exercise schema 1 plus schema 2 round trips against the real server. The default command uses the reference submodule and runs the schema 1 matrix. No diary content or audio is included. Format-level proof does not establish phone, catalog, native File Provider or packaged release behavior.
