# Atomic Editor for Illustrated diary

Status: Proposal

Research requested for P1-M34 on 2026-11-13: can Atomic Editor provide an Obsidian-style inline Markdown surface with images? Yes, technically, with an app-owned image adapter and editing guards. Adoption remains a proposal; the current plans put format/creation in [M34](../archive/milestones/p1-m34/milestone-34-illustrated-diary-overview.md) and the read-only GUI in [M35](../archive/milestones/p1-m35/milestone-35-illustrated-diary-gui-overview.md).

## M35 adoption update

The owner approved the rendered read-only viewer and dependency qualification on 2026-11-13. M35 now composes the pinned published package with strict app mutation guards and app-owned photos, without the upstream wrapper, image/table widgets or global stylesheet. The [implementation evidence](../archive/milestones/p1-m35/milestone-35-illustrated-diary-gui-evidence.md) supersedes the immediate-viewer proposal below; editable live preview remains a separate proposal.

## Evidence and conclusion

Inspected upstream `main` at commit [`b6ed65f01bde4510031bc7a495520bc1a7688c66`](https://github.com/kenforthewin/atomic-editor/tree/b6ed65f01bde4510031bc7a495520bc1a7688c66), dated 2026-07-11, whose package manifest declares `@atomic-editor/editor` version `0.6.2`. This is a standalone React/CodeMirror library, not an Obsidian plugin requiring Obsidian. The inspected source has no Tauri dependency. Context7 returned no matching library; the assessment uses upstream source instead.

**Recommendation:** qualify the exported CodeMirror extensions inside our existing editor adapter. Keep document identity, retained state, autosave, resource limits and paragraph review under their existing owners. For M35, use a strictly immutable reading surface and an app-owned photo widget. Broader editable live preview is a separate scope decision: editing Illustrated entries is currently excluded by both milestone plans.

This review downloaded source into a temporary directory and inspected code, tests and documentation. It did not install the library, verify the npm tarball, execute upstream tests or run a native GUI prototype. Compatibility below is a source-level assessment, not runtime acceptance.

## Fit

| Requirement          | Finding                                                                                                                                                                                                                                                                                                            |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| React + CodeMirror 6 | Matches our stack. [Peer ranges](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/package.json) accept React 18/19 and CM6; our React 19 and CM6 pins fall within them.                                                                                                 |
| Inline Markdown      | [`inlinePreview()`](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/src/inline-preview.ts) decorates raw Markdown and reveals syntax while editing. Headings, emphasis, links, lists and task checkboxes are supported; tables have a separate widget.                 |
| Portable files       | Decorations leave the Markdown source intact. Our `![alt](YYYY-MM-DD/01.png)` format fits the image parser's simple URL form. Keep durable paths relative.                                                                                                                                                         |
| Photos               | [`imageBlocks()`](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/src/image-blocks.ts) creates block images with alt text and browser lazy loading. Images remain visible while their source is edited.                                                                |
| Composition          | [Public exports](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/src/index.ts) include preview, images, tables, themes, Markdown highlight syntax and reading-mode extensions. We can compose selected pieces.                                                         |
| License              | The actual [MIT license](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/LICENSE) permits reuse with retention of its copyright/permission notice. It fits our Apache-2.0 compatibility requirement; exact dependency/notice review is still required before shipping. |

## Integration gaps

### Authenticated local photos

`imageBlocks()` accepts no resolver configuration: it assigns the Markdown destination directly to `img.src`. Relative paths therefore resolve against the webview document, not the diary's filesystem directory. It cannot attach our server session authorization or invoke a Tauri command. Its table image code also assigns `img.src` directly.

Proposed integration:

1. Give the viewer the validated Markdown body and photo manifest, excluding YAML metadata/prompts from the rendered body.
2. Resolve only the M34-approved `<date>/NN.png` references through the authenticated photo bridge. Refuse arbitrary file, network and traversal destinations.
3. Create temporary object URLs from returned PNG bytes and supply them to an app-owned widget. Refresh widgets through effects when bytes arrive; keep source paths and offsets unchanged.
4. Release bytes and revoke URLs on tab disposal, replacement and stale asynchronous completion. Bound concurrent loads and decoded image memory as well as transport bytes. Browser `loading="lazy"` alone does not defer bridge requests made by our code.

The current [Tauri CSP](../../src-tauri/tauri.conf.json) permits `img-src 'self' data:` but **does not permit `blob:`**. The M35 object-URL proposal needs a narrow CSP change and packaged proof. Do not open broad filesystem or remote-image access. If tables are included, route their image paths through the same resolver or disable table image rendering; replacing only block images leaves another image-loading path.

The upstream dimension cache is module-global and has no eviction API. Our widget should own its dimensions/cache lifecycle. Its image regex intentionally omits some Markdown edge cases, such as escaped parentheses; this is acceptable for the constrained generated paths, but not evidence of general Markdown image conformance. There is no public image-upload, clipboard-file or file-drop persistence contract; adding user images would require app storage policy and additional scope.

### Strict read-only behavior

Upstream [`readOnlyExtension(true)`](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/src/read-only.ts) disables ordinary editing and makes the whole surface stay rendered, but deliberately leaves task checkboxes toggleable. The [test suite](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/src/__tests__/read-only.test.tsx) explicitly expects those toggles to change Markdown.

For M35, disable checkbox interaction and reject document-changing transactions at the viewer boundary, while allowing selection/search and reconfiguration. CM6's read-only facet alone does not reject arbitrary explicit dispatches. Test source immutability after checkbox clicks, undo, paste, drop and table actions. Provide a deliberate owner-controlled load path for replacement versions.

### Existing editor ownership

The upstream [React wrapper](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/src/AtomicCodeMirrorEditor.tsx) owns its state/view, consumes `markdownSource` at mount and remounts on document-identity changes. Extensions are captured at mount; changing their array does not reconfigure the view. Its public handle does not expose retained EditorState replacement.

Our [ResourceBuffer](../../src/editor/resource-buffer.ts) retains state/history across view detach/attach, enforces budgets, guards edits during review and tracks IME composition. Replacing it with the wrapper would require reconciling these contracts. Compose extensions in `src/editor` instead; keep React/CM/DOM imports out of `src/application`. Do not layer preview and review replacements blindly over the same ranges; qualify them separately and suspend preview during review if needed.

Use app CSS tokens rather than importing a second global theme indiscriminately. Route `onLinkClick` through an approved app action with URL validation; the upstream default is `window.open`. Ensure exactly one resolved CM state/view installation. Adoption adds the Atomic package and promotes relevant CM/Lezer peers to explicit direct dependencies; optional fenced-language packages are unnecessary for the initial viewer. No manifest or lockfile change is made by this research.

## Upstream maturity and qualification

The project has unit/DOM tests, a package-consumer build test, Chromium coverage and Firefox/WebKit smoke configurations; see its [testing guide](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/docs/testing.md). These are useful foundations, not proof of our WKWebView, IME, autosave or paragraph-review integration. The [changelog](https://github.com/kenforthewin/atomic-editor/blob/b6ed65f01bde4510031bc7a495520bc1a7688c66/CHANGELOG.md) warns that pre-1.0 minor releases can break the API.

As observed on 2026-11-13, upstream has open reports for [spaces disappearing/caret movement in tables (#18)](https://github.com/kenforthewin/atomic-editor/issues/18) and [whole-logical-line source reveal (#26)](https://github.com/kenforthewin/atomic-editor/issues/26). A [table-space fix (#19)](https://github.com/kenforthewin/atomic-editor/pull/19) and [reference-link/caret fix (#23)](https://github.com/kenforthewin/atomic-editor/pull/23) are open PRs. Treat these as reported risks, not locally reproduced defects or shipped fixes. Long single-line diary paragraphs particularly warrant a reveal-behavior check.

Before adopting:

- Approve the dependency and choose a pinned version; inspect the actual package contents, resolved licenses and bundle, without changing our build tools to match upstream's development tooling.
- Prove read-only text plus three local PNGs in the packaged app, no remote requests, failed-image fallback, alt text and lazy bridge loading.
- Prove URL cleanup and stale-load handling across close, reopen, date switch and replacement, with bounded memory and stable scroll.
- For editable preview, additionally prove exact Markdown round trips, retained undo/selection, CJK IME, autosave, keyboard shortcuts, paragraph review and budget refusals. Test tables against the reported space/caret issue.
- Run the repository gate and affected native smoke. Do not mark M34/M35 implemented from this research.

For the immediate read-only M35 viewer, a small native CM preview/photo adapter remains an alternative with no new package. Atomic offers more value if the owner also wants editable live preview for existing diary buffers; its photo transport still requires our adapter in either case.

## Repository validation

Documentation formatting, local link targets and `git diff --check` pass. The final full gate (`run.CGySh3`, 2026-11-13) passed G01, G02 and G04–G07, but G03 failed in the existing `image::tests::rejects_unknown_critical_and_invalid_palette_chunks`: an `abca` PNG chunk was accepted where the test expected `InvalidOutput`. An earlier run also hit an image-engine lifecycle timeout; that test passed when rerun alone. The initial sandboxed run could not bind loopback for bridge tests; subsequent runs had loopback access. No application code was changed by this research, and a passing full gate is not claimed.
