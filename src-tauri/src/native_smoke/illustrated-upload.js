/* global window, document, setTimeout, location */
(async () => {
  if (window.__illustratedSmoke) return;
  window.__illustratedSmoke = { state: "running", step: "connect" };
  const wait = async (predicate, message) => {
    for (let attempt = 0; attempt < 1200; attempt++) {
      const result = predicate();
      if (result) return result;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    throw Error(message);
  };
  const button = (name) =>
    [...document.querySelectorAll("button")].find(
      (b) =>
        b.getAttribute("aria-label") === name || b.textContent.trim() === name,
    );
  const check = (condition, message) => {
    if (!condition) throw Error(message);
  };
  const upload = () => button("Upload Illustrated Diary to Notion");
  try {
    check(location.origin === "tauri://localhost", "packaged origin");
    const day = await wait(
      () => document.querySelector('button[data-date][aria-selected="true"]'),
      "selected date",
    );
    const date = day.dataset.date;
    await wait(
      () => upload() && !upload().disabled,
      "published upload availability without Authentic/profile/engine",
    );
    check(
      button("Create Empty") && !button("Create Empty").disabled,
      "Authentic remains virtual",
    );
    check(
      !document.querySelector('[aria-label="Illustrated diary viewer"]'),
      "viewer is not required",
    );
    check(
      button("Create Illustrated Diary")?.disabled,
      "creation unavailable without an AI profile",
    );
    const icon = upload().querySelector("span");
    check(icon, "distinct bitmap upload icon");
    window.__illustratedSmoke.step = "upload-and-navigate";
    upload().click();
    await wait(
      () => button("Uploading Illustrated Diary…")?.disabled,
      "pending upload action",
    );
    const other = [...document.querySelectorAll("button[data-date]")].find(
      (b) => b.dataset.date !== date,
    );
    check(other, "another calendar date");
    other.click();
    const otherDate = other.dataset.date;
    await wait(
      () =>
        document.querySelector(
          `button[data-date="${otherDate}"][aria-selected="true"]`,
        ),
      "navigate during upload",
    );
    await wait(
      () =>
        document
          .querySelector('[aria-label="Global Messages"]')
          ?.textContent.includes(
            `Exported Illustrated diary ${date} to Notion.`,
          ),
      "upload status completion",
    );
    check(
      document.querySelector(
        `button[data-date="${otherDate}"][aria-selected="true"]`,
      ),
      "completion preserves selected date",
    );
    check(
      !document.querySelector('[aria-label="Illustrated diary viewer"]'),
      "completion does not open viewer",
    );
    check(
      upload()?.disabled && upload().title.includes("no published Illustrated"),
      "missing entry guard",
    );
    window.__illustratedSmoke = {
      state: "passed",
      date,
      packagedOrigin: true,
      virtualAuthentic: true,
      noViewer: true,
      pendingGuard: true,
      navigationPreserved: true,
      missingEntryGuard: true,
    };
  } catch (error) {
    window.__illustratedSmoke = {
      state: "failed",
      step: window.__illustratedSmoke.step,
      error: String(error?.message || "native assertion failed"),
    };
  }
})();
