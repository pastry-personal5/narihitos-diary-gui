import { expect, it } from "vitest";
import { assertVersions } from "../scripts/version-policy.mjs";
it("rejects deliberate package, Cargo and Tauri drift", () => {
  const cargo = '[workspace.package]\nversion = "1.0.0"\n';
  expect(
    assertVersions({ version: "1.0.0" }, cargo, { version: "1.0.0" }),
  ).toBe("Version v1.0.0");
  expect(() =>
    assertVersions({ version: "1.0.1" }, cargo, { version: "1.0.0" }),
  ).toThrow(/drift/u);
  expect(() =>
    assertVersions({ version: "1.0.0" }, cargo.replace("1.0.0", "1.0.1"), {
      version: "1.0.0",
    }),
  ).toThrow(/drift/u);
  expect(() =>
    assertVersions({ version: "1.0.0" }, cargo, { version: "1.0.1" }),
  ).toThrow(/drift/u);
});
