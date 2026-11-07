import { expect, it } from "vitest";
import { jobPageDto } from "../src/adapters/activity-validation";

const job = { id: "job-1", sequence: 1, state: "running" };
const page = { jobs: [job], next_cursor: null, event_cursor: 0 };

it("preserves bounded discovery and its replay watermark", () => {
  expect(jobPageDto(page)).toEqual(page);
  expect(
    jobPageDto({ ...page, jobs: [], next_cursor: "job-2" }).next_cursor,
  ).toBe("job-2");
});

it.each([
  { ...page, jobs: [job, job] },
  { ...page, jobs: Array.from({ length: 201 }, () => job) },
  { ...page, event_cursor: -1 },
  { ...page, event_cursor: Number.MAX_SAFE_INTEGER + 1 },
  { ...page, next_cursor: "../private" },
  { ...page, jobs: [{ ...job, sequence: 0 }] },
  { ...page, jobs: [{ ...job, state: "invented" }] },
])("rejects malformed discovery before retention", (value) => {
  expect(() => jobPageDto(value)).toThrow();
});

import { streamEvent } from "../src/application/events";
const progress = {
  kind: "chat",
  job_id: "job-1",
  request_id: "r",
  instance_id: "i",
  session_id: "s",
  chat_id: "c",
  origin_date: null,
  profile_id: "main",
  locale: "en-US",
  phase: "waiting_for_output",
  elapsed_ms: 0,
  last_agent_activity_at: null,
  cancellation_requested: false,
  error_code: null,
};
it("preserves chat discovery and refuses cross-session snapshot metadata", () => {
  const enriched = { ...job, progress };
  expect(jobPageDto({ ...page, jobs: [enriched] }).jobs[0]).toEqual(enriched);
  const event = {
    kind: "job_snapshot",
    instance_id: "i",
    session_id: "s",
    first: true,
    complete: true,
    jobs: [enriched],
  };
  expect(streamEvent(event)).toMatchObject({ jobs: [enriched] });
  expect(() => streamEvent({ ...event, session_id: "other" })).toThrow();
});
it.each([
  { phase: "invented" },
  { job_id: "other" },
  { elapsed_ms: -1 },
  { locale: "" },
  { error_code: "secret" },
  { origin_date: "2026-02-30" },
  { cancellation_requested: "false" },
])("rejects malformed progress at discovery", (patch) => {
  expect(() =>
    jobPageDto({
      ...page,
      jobs: [{ ...job, progress: { ...progress, ...patch } }],
    }),
  ).toThrow();
});

import { generationView } from "./generation-fixtures";
it("retains generation metadata and rejects inconsistent counts and candidate identities", () => {
  const v = generationView();
  const wire = { ...v.job, progress: v.progress };
  expect(jobPageDto({ ...page, jobs: [wire] }).jobs[0]).toEqual(wire);
  for (const patch of [
    { ready_count: 5 },
    { candidate_ids: ["other-1"] },
    { candidate_index: 6 },
    { baseline_revision: { sequence: 0, sha256: "x" } },
  ]) {
    expect(() =>
      jobPageDto({
        ...page,
        jobs: [{ ...wire, progress: { ...v.progress, ...patch } }],
      }),
    ).toThrow();
  }
});
it("drops unsolicited bodies from projected metadata", () => {
  const parsed = jobPageDto({
    ...page,
    jobs: [{ ...job, progress: { ...progress, provisional: "private text" } }],
  });
  expect(JSON.stringify(parsed)).not.toContain("private text");
});
