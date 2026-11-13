/* global window, document, setTimeout, clearTimeout, URL, location, crypto, Event, Image */
(async () => {
  if (window.__illustratedSmoke) return;
  window.__illustratedSmoke = { state: "running", step: "connect" };
  const report = (step) => {
    window.__illustratedSmoke = { state: "running", step };
  };
  const check = (value, message) => {
    if (!value) throw Error(message);
  };
  const wait = async (predicate, message) => {
    for (let attempt = 0; attempt < 1200; attempt++) {
      const value = await predicate();
      if (value) return value;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    throw Error(message);
  };
  const button = (name) =>
    [...document.querySelectorAll("button")].find(
      (b) =>
        b.getAttribute("aria-label") === name || b.textContent.trim() === name,
    );
  const click = async (name) => {
    (
      await wait(() => {
        const b = button(name);
        if (b?.disabled) window.__illustratedSmoke.reason = b.title;
        return b && !b.disabled && b;
      }, `button unavailable: ${name}`)
    ).click();
  };
  const invoke = window.__TAURI_INTERNALS__.invoke;
  let created = 0,
    revoked = 0;
  const originalCreate = URL.createObjectURL.bind(URL),
    originalRevoke = URL.revokeObjectURL.bind(URL);
  URL.createObjectURL = (blob) => {
    created++;
    return originalCreate(blob);
  };
  URL.revokeObjectURL = (url) => {
    revoked++;
    return originalRevoke(url);
  };
  try {
    check(location.origin === "tauri://localhost", "packaged origin");
    await wait(
      () => document.querySelector("button[data-date]"),
      "automatic connection",
    );
    const day = [...document.querySelectorAll("button[data-date]")].find(
      (b) => b.dataset.date !== "2026-11-13",
    );
    check(day, "isolated unsaved date");
    const date = day.dataset.date;
    await wait(() => {
      const target = document.querySelector(`button[data-date="${date}"]`);
      if (target?.getAttribute("aria-selected") === "true") return true;
      target?.click();
      return false;
    }, "choose isolated date after initial navigation");
    await wait(
      () => button("Create Empty") && !button("Create Empty").disabled,
      "unsaved date readiness",
    );
    check(
      button("Create Illustrated Diary")?.disabled,
      "unsaved date must disable creation",
    );
    report("save-source");
    const initial = await invoke("load_authentic", { date });
    await invoke("save_authentic", {
      date,
      operationId: crypto.randomUUID(),
      expectedRevision: initial.revision,
      body: "Morning tea. An afternoon walk.\n",
      bufferGeneration: 1,
    });
    report("reload-source");
    await click(`Close ${date}`);
    await wait(
      () => !button(`Close ${date}`),
      "initial tab close acknowledgement",
    );
    document.querySelector(`button[data-date="${date}"]`).click();
    await wait(
      () => button("Create Empty")?.title.includes("already exists"),
      "saved Authentic reload",
    );
    report("engines");
    await click("Settings");
    await click("AI Agents");
    await click("Check Synthetic");
    await wait(
      () =>
        document
          .querySelector('[aria-label="Image engines"]')
          ?.textContent.includes("ready"),
      "engine probe readiness",
    );
    check(
      document
        .querySelector('[aria-label="Image engines"]')
        ?.textContent.includes("Selected for Illustrated"),
      "effective engine selection",
    );
    await click("Go back");
    report("create");
    await click("Create Illustrated Diary");
    await wait(
      () => document.querySelector('[aria-label="Illustrated diary viewer"]'),
      "creation completion opens viewer",
    );
    const first = await invoke("read_illustrated", { date });
    check(first.photos.length >= 1, "photo manifest");
    async function loadedPhotos() {
      await wait(
        () =>
          document.querySelectorAll(".illustrated-photo").length ===
          first.photos.length,
        "ordered photo widgets",
      );
      for (let index = 0; index < first.photos.length; index++) {
        const image = await wait(() => {
          const figure = document.querySelectorAll(".illustrated-photo")[index];
          figure?.scrollIntoView({ block: "center" });
          const img = figure?.querySelector("img");
          return img && img.complete && img.naturalWidth > 0 && img;
        }, "native PNG decode");
        check(
          image.src.startsWith("blob:"),
          "authenticated local image transport",
        );
        check(image.alt.length > 0, "photo alt text");
      }
      check(
        document
          .querySelector(".illustrated-host .cm-content")
          ?.getAttribute("contenteditable") !== "true",
        "immutable viewer",
      );
      check(
        document.querySelector(".illustrated-host .cm-atomic-h1") ||
          document.querySelector(".illustrated-host .cm-content")?.textContent
            .length > 0,
        "rendered diary text",
      );
    }
    await loadedPhotos();
    report("source-change");
    const source = await invoke("load_authentic", { date });
    await invoke("save_authentic", {
      date,
      operationId: crypto.randomUUID(),
      expectedRevision: source.revision,
      body: source.body + "Evening reflection.\n",
      bufferGeneration: 2,
    });
    window.dispatchEvent(new Event("focus"));
    await wait(
      () =>
        document
          .querySelector('[aria-label="Global Messages"]')
          ?.textContent.includes("has changed since this Illustrated"),
      "source-change Global Message",
    );
    report("replace");
    await click("Create Illustrated Diary");
    await wait(async () => {
      const entry = await invoke("read_illustrated", { date });
      return entry.version !== first.version;
    }, "successful replacement publication");
    await wait(() => revoked >= first.photos.length, "replacement URL release");
    await loadedPhotos();
    report("background");
    await click("Create Illustrated Diary");
    await click("Date tab");
    const other = [...document.querySelectorAll("button[data-date]")].find(
      (b) => b.dataset.date !== date,
    );
    const otherDate = other.dataset.date;
    await wait(() => {
      const target = document.querySelector(`button[data-date="${otherDate}"]`);
      if (target?.getAttribute("aria-selected") === "true") return true;
      target?.click();
      return false;
    }, "background navigation");
    await wait(
      () =>
        document
          .querySelector('[aria-label="Global Messages"]')
          ?.textContent.includes("Open it from the Illustrated tab"),
      "background completion Global Message",
    );
    check(
      !document
        .querySelector('button[role="tab"][aria-selected="true"]')
        ?.textContent.includes("Illustrated"),
      "background completion preserves focus/date",
    );
    await click("Illustrated tab");
    await click(`${date} · Illustrated`);
    await loadedPhotos();
    report("reconnect");
    await click("Settings");
    await click("Server");
    await click("Connect");
    await wait(
      () =>
        document.querySelector('[aria-label="Server connection status"]')
          ?.textContent === "Connected",
      "same-lease reconnect",
    );
    await click("Go back");
    await loadedPhotos();
    report("close-reopen");
    await click(`Close ${date} · Illustrated`);
    await wait(() => created === revoked, "close releases all object URLs");
    await click("Illustrated tab");
    await click(`${date} · Illustrated`);
    await wait(
      () => document.querySelector('[aria-label="Illustrated diary viewer"]'),
      "Explorer reopen",
    );
    await loadedPhotos();
    const image = document.querySelector(".illustrated-photo img");
    image.dispatchEvent(new Event("error"));
    await wait(
      () =>
        document
          .querySelector(".illustrated-photo")
          ?.textContent.includes("Photo unavailable"),
      "photo failure fallback",
    );
    const retry = document.querySelector(".illustrated-photo button");
    retry.click();
    await wait(() => {
      const i = document.querySelector(".illustrated-photo img");
      return i?.complete && i.naturalWidth > 0;
    }, "photo Retry");
    report("csp");
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(
        () => reject(Error("missing remote image CSP refusal")),
        3000,
      );
      const listener = (event) => {
        if (
          event.violatedDirective === "img-src" &&
          event.blockedURI.startsWith("http://127.0.0.1:1")
        ) {
          clearTimeout(timeout);
          document.removeEventListener("securitypolicyviolation", listener);
          resolve();
        }
      };
      document.addEventListener("securitypolicyviolation", listener);
      const probe = new Image();
      probe.src = "http://127.0.0.1:1/illustrated-smoke-denied.png";
    });
    report("delete");
    await click("Delete Illustrated diary");
    check(
      document.activeElement?.textContent === "Cancel",
      "destructive confirmation initial focus",
    );
    await click("Cancel");
    check(
      (await invoke("read_illustrated", { date })).photos.length > 0,
      "cancel confirmation preserves publication",
    );
    await click("Delete Illustrated diary");
    const dialog = document.querySelector(
      '[role="dialog"][aria-label="Delete Illustrated diary"]',
    );
    check(dialog, "delete confirmation dialog");
    [...dialog.querySelectorAll("button")]
      .find((b) => b.textContent === "Delete Illustrated diary")
      .click();
    await wait(
      () => !document.querySelector('[aria-label="Illustrated diary viewer"]'),
      "delete closes viewer",
    );
    await wait(() => created === revoked, "delete releases all object URLs");
    await wait(
      () =>
        !document.querySelector(
          '[role="dialog"][aria-label="Delete Illustrated diary"]',
        ),
      "delete confirmation closes",
    );
    await click("Illustrated tab");
    await wait(
      () => !button(`${date} · Illustrated`),
      "deleted date removed from Explorer catalog",
    );
    let gone = false;
    try {
      await invoke("read_illustrated", { date });
    } catch (error) {
      gone = error.code === "not_found";
    }
    check(gone, "deleted entry is absent");
    check(
      (await invoke("load_authentic", { date })).body.includes(
        "Evening reflection",
      ),
      "Authentic preserved",
    );
    window.__illustratedSmoke = {
      state: "passed",
      photos: first.photos.length,
      urlsCreated: created,
      urlsRevoked: revoked,
      packagedOrigin: true,
      remoteImageBlocked: true,
    };
  } catch (error) {
    window.__illustratedSmoke = {
      state: "failed",
      step: window.__illustratedSmoke.step,
      error: String(error?.message || "native assertion failed"),
    };
  }
})();
