# Third-party notices

Status: Active

Dependency code retains its upstream licenses; the project's Apache-2.0 license applies to project-authored code. Exact dependency declarations, selected alternatives, authors where supplied, source archives and preserved legal text are recorded in the [resolved inventory](assets/third-party/inventory.json).

The inventory covers 429 Cargo and 282 npm resolved package records, including optional platform/build/test dependencies. It preserves 389 distinct notice files under `assets/third-party/licenses/`, keyed by SHA-256. All Cargo records and installed npm packages have collected notice assets; optional npm artifacts absent on this Mac are identified as uninstalled. No upstream code was modified or copied into application source.

For dual/multiple alternatives the inventory selects MIT when offered, otherwise Apache-2.0 when offered, otherwise the declared alternative. Required `AND` combinations retain every component. No LGPL alternative is selected. BSD, ISC, Zlib, Unicode-3.0, BlueOak-1.0.0, MIT-0, CC0-1.0, CC-BY-4.0, MPL-2.0 and the LLVM exception retain their declared terms and attribution. These notices do not relicense third-party code as Apache-2.0.

MPL-covered sources remain unmodified; exact source-archive links are provided per package. License texts/data attribution and combined notices are preserved. Standard MIT/Apache/MPL terms are included alongside original package notices when published packages reference those standard licenses without bundling separate texts. Original README/source notice sections and published author/license declarations are retained rather than inventing copyright holders.

[Supplement provenance](assets/third-party/supplemental-notices.json) identifies exact-commit upstream URLs, same-version official native-binary wrapper notices, standard-license text sources and original source/README sections. The upstream objc2 repository notice documents its SDK-derived-source licensing caveat; this record preserves that notice and does not claim independent Apple SDK/license clearance. The chosen personal macOS/Tauri scope and pending broader platform/release qualification remain as recorded in the project architecture.

Regenerate with the existing Python/Cargo tools:

```sh
python3 scripts/complete-local-notices.py
python3 scripts/collect-notices.py
```

`fetch-notice-supplements.py` optionally retrieves omitted repository-level files from the exact Git SHA in crate metadata; checked-in supplements make ordinary regeneration offline. Raw legal text is preserved byte-for-byte and excluded from formatting. The Tauri bundle configuration includes this document and the complete inventory/license directory; standalone server copies must travel with these same notices. Public distribution remains outside this milestone.
