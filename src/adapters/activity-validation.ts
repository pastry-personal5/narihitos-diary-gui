import type { JobPage } from "../application/bridge";
import { importJobDto } from "./navigation-validation";

/** Keep discovery bounded before any records enter application state. */
export function jobPageDto(value: unknown): JobPage {
  if (typeof value !== "object" || value === null)
    throw new Error("Invalid job page");
  const page = value as Record<string, unknown>;
  if (
    !Array.isArray(page.jobs) ||
    page.jobs.length > 200 ||
    !Number.isSafeInteger(page.event_cursor) ||
    (page.event_cursor as number) < 0 ||
    (page.next_cursor !== null &&
      (typeof page.next_cursor !== "string" ||
        !/^[a-zA-Z0-9_-]{1,64}$/u.test(page.next_cursor)))
  )
    throw new Error("Invalid job page");
  const jobs = page.jobs.map(importJobDto);
  if (new Set(jobs.map((job) => job.id)).size !== jobs.length)
    throw new Error("Invalid job page");
  return {
    jobs,
    next_cursor: page.next_cursor as string | null,
    event_cursor: page.event_cursor as number,
  };
}
