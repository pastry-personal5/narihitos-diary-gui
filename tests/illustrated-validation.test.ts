import { expect, it } from "vitest";
import {
  illustratedDatesDto,
  illustratedEntryDto,
  illustratedPhotoDto,
  imageEnginesDto,
} from "../src/adapters/illustrated-validation";
import { aiProgress } from "../src/application/ai-activities/progress";
import { jobPageDto } from "../src/adapters/activity-validation";
import { streamEvent } from "../src/application/events";
import {
  illustratedEntry,
  illustratedDate,
  illustratedProgress,
  imageEngines,
  png,
} from "./illustrated-fixtures";
it("validates Illustrated content, manifest, date order and effective engine identity", () => {
  expect(illustratedEntryDto(illustratedEntry(), illustratedDate)).toEqual(
    illustratedEntry(),
  );
  for (const patch of [
    { date: "2026-02-30" },
    { version: "../private" },
    { source_revision: 0 },
    { source_sha256: "not a hash" },
    { theme_count: 0 },
    { photos: [] },
    { photos: [{ file: "../01.png", seed: 0, prompt: "private" }] },
    { photos: [{ file: "01.png", seed: -1, prompt: "private" }] },
    { markdown: "a".repeat(1024 * 1024 + 1) },
  ])
    expect(() =>
      illustratedEntryDto({ ...illustratedEntry(), ...patch }, illustratedDate),
    ).toThrow();
  expect(
    illustratedDatesDto({ dates: ["2026-11-12", illustratedDate] }),
  ).toHaveLength(2);
  for (const dates of [
    [illustratedDate, illustratedDate],
    [illustratedDate, "2026-11-12"],
    ["2026-02-30"],
  ])
    expect(() => illustratedDatesDto({ dates })).toThrow();
  expect(imageEnginesDto(imageEngines())).toEqual(imageEngines());
  expect(() =>
    imageEnginesDto({ ...imageEngines(), illustrated_engine: "other" }),
  ).toThrow();
  expect(() =>
    imageEnginesDto({
      ...imageEngines(),
      image_engines: [
        ...imageEngines().image_engines,
        ...imageEngines().image_engines,
      ],
    }),
  ).toThrow();
  expect(illustratedPhotoDto([...png()])).toEqual(png());
  expect(() => illustratedPhotoDto([137, 80])).toThrow();
});
it("accepts Illustrated discovery/replay beside ordinary and chat jobs and binds session scope", () => {
  const illustrated = {
    id: "job",
    sequence: 1,
    state: "running",
    progress: illustratedProgress(),
  };
  const ordinary = { id: "ordinary", sequence: 1, state: "queued" };
  expect(
    jobPageDto({
      jobs: [ordinary, illustrated],
      event_cursor: 8,
      next_cursor: null,
    }).jobs,
  ).toHaveLength(2);
  const snapshot = {
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    jobs: [ordinary, illustrated],
    first: true,
    complete: true,
  };
  expect(streamEvent(snapshot)).toEqual(snapshot);
  expect(() => streamEvent({ ...snapshot, session_id: "foreign" })).toThrow();
  expect(
    aiProgress({ ...illustratedProgress(), secret: "discarded" }, "job"),
  ).toEqual(illustratedProgress());
  for (const patch of [
    { job_id: "other" },
    { photo_index: 1 },
    { photo_count: 100 },
    { phase: "photo", photo_index: 0 },
    { origin_date: "2026-02-30" },
    { error_code: "private" },
  ])
    expect(() =>
      aiProgress({ ...illustratedProgress(), ...patch }, "job"),
    ).toThrow();
});
