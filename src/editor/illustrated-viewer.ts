import { searchKeymap } from "@codemirror/search";
import { keymap } from "@codemirror/view";
import { StateField, type Extension } from "@codemirror/state";
import { Decoration, EditorView, WidgetType } from "@codemirror/view";
import {
  inlinePreview,
  readOnlyExtension,
  atomicMarkdownSyntax,
} from "@atomic-editor/editor";
import type { IllustratedEntry } from "../application/illustrated/contracts";
import type {
  IllustratedPhotos,
  PhotoLease,
} from "../adapters/illustrated-photos";

class PhotoWidget extends WidgetType {
  private cleanup = new WeakMap<HTMLElement, () => void>();
  constructor(
    private entry: IllustratedEntry,
    private n: number,
    private alt: string,
    private photos: IllustratedPhotos | undefined,
    private online: boolean,
  ) {
    super();
  }
  toDOM() {
    const figure = document.createElement("figure");
    figure.className = "illustrated-photo";
    const status = document.createElement("p");
    const image = document.createElement("img");
    image.alt = this.alt;
    image.loading = "lazy";
    image.hidden = true;
    const retry = document.createElement("button");
    retry.type = "button";
    retry.textContent = "Retry photo";
    retry.setAttribute("aria-label", `Retry photo ${this.n}: ${this.alt}`);
    retry.hidden = true;
    figure.append(image, status, retry);
    let active: AbortController | null = null;
    let lease: PhotoLease | null = null;
    const unload = () => {
      active?.abort();
      active = null;
      lease?.release();
      lease = null;
      image.hidden = true;
      image.removeAttribute("src");
    };
    const fallback = () => {
      unload();
      status.textContent = `Photo ${this.n}: ${this.alt}. Photo unavailable.`;
      retry.hidden = !this.online;
    };
    const load = async () => {
      if (active || lease || !this.online || !this.photos) return;
      const request = new AbortController();
      active = request;
      status.textContent = `Loading photo ${this.n}: ${this.alt}`;
      retry.hidden = true;
      try {
        const loaded = await this.photos.acquire(
          this.entry.date,
          this.entry.version,
          this.n,
          request.signal,
        );
        if (request.signal.aborted || active !== request) {
          loaded.release();
          return;
        }
        lease = loaded;
        image.width = loaded.width;
        image.height = loaded.height;
        image.onload = () => {
          if (active === request) status.textContent = "";
        };
        image.onerror = fallback;
        image.src = loaded.url;
        image.hidden = false;
      } catch {
        if (!request.signal.aborted && active === request) fallback();
      }
    };
    retry.onclick = () => void load();
    status.textContent = this.online
      ? `Photo ${this.n}: ${this.alt}`
      : `Photo ${this.n}: ${this.alt}. Reconnect to load.`;
    // Viewport demand controls bridge reads; browser loading=lazy alone does not.
    const observer =
      typeof IntersectionObserver !== "undefined"
        ? new IntersectionObserver(
            (records) => {
              if (records.some((r) => r.isIntersecting)) void load();
              else {
                unload();
                status.textContent = `Photo ${this.n}: ${this.alt}`;
              }
            },
            { rootMargin: "200px" },
          )
        : null;
    observer?.observe(figure);
    if (!observer) {
      retry.hidden = !this.online;
      retry.textContent = "Load photo";
    }
    this.cleanup.set(figure, () => {
      observer?.disconnect();
      unload();
    });
    return figure;
  }
  destroy(dom: HTMLElement) {
    this.cleanup.get(dom)?.();
    this.cleanup.delete(dom);
  }
  ignoreEvent() {
    return true;
  }
}
/** The upstream image/table widgets are deliberately omitted: every image is resolved by the app. */
export function inertIllustratedControls(view: EditorView) {
  view.dom
    .querySelectorAll<HTMLInputElement>('input[type="checkbox"]')
    .forEach((input) => {
      input.disabled = true;
      input.tabIndex = -1;
    });
}
export function illustratedViewer(
  entry: IllustratedEntry,
  photos: IllustratedPhotos | undefined,
  online: boolean,
): Extension {
  return [
    readOnlyExtension(true),
    keymap.of(searchKeymap),
    EditorView.updateListener.of((update) =>
      inertIllustratedControls(update.view),
    ),
    atomicMarkdownSyntax,
    inlinePreview({
      onLinkClick: () => {
        /* Links are selectable/copyable; opening external content is not an Illustrated action. */
      },
    }),
    EditorView.editorAttributes.of({
      class: "illustrated-reading-view",
      tabindex: "0",
      "aria-label": `${entry.date} Illustrated diary · read-only`,
    }),
    EditorView.domEventHandlers({
      click: (event) => {
        if ((event.target as HTMLElement).closest('input[type="checkbox"]')) {
          event.preventDefault();
          return true;
        }
        return false;
      },
    }),
    StateField.define({
      create(state) {
        const ranges = [];
        for (let i = 1; i <= state.doc.lines; i++) {
          const line = state.doc.line(i);
          const match =
            /^!\[((?:\\.|[^\]])*)\]\((\d{4}-\d{2}-\d{2})\/(\d{2}\.png)\)$/u.exec(
              line.text,
            );
          if (!match || match[2] !== entry.date) continue;
          const n = entry.photos.findIndex((p) => p.file === match[3]) + 1;
          if (n < 1) continue;
          const alt = match[1].replace(/\\([\\[\]])/gu, "$1");
          ranges.push(
            Decoration.widget({
              widget: new PhotoWidget(entry, n, alt, photos, online),
              block: true,
              side: 1,
            }).range(line.to),
          );
        }
        return Decoration.set(ranges);
      },
      update(value) {
        return value;
      },
      provide: (field) => EditorView.decorations.from(field),
    }),
    EditorView.theme({
      ".cm-content": { padding: "1rem", lineHeight: "1.7" },
      ".cm-atomic-strong": { fontWeight: "bold" },
      ".cm-atomic-em": { fontStyle: "italic" },
      ".cm-atomic-task-checkbox": { pointerEvents: "none" },
      ".cm-atomic-hr": { borderBottom: "1px solid currentColor" },
    }),
  ];
}
