import { describe, expect, test } from "bun:test";

import {
  findLatestReleaseTag,
  incrementPatchVersion,
  resolveReleaseVersion,
} from "./resolve-release-version.mjs";

describe("findLatestReleaseTag", () => {
  test("ignores non-stable tags and returns the highest stable version", () => {
    expect(findLatestReleaseTag(["draft", "v1.2.9", "v1.10.0", "v1.10.0-rc1"]))
      .toBe("v1.10.0");
  });
});

describe("incrementPatchVersion", () => {
  test("increments the patch component", () => {
    expect(incrementPatchVersion("v3.4.5")).toBe("v3.4.6");
  });
});

describe("resolveReleaseVersion", () => {
  test("reuses the existing stable head tag", () => {
    expect(resolveReleaseVersion({
      headTags: ["v1.2.3"],
      allTags: ["v1.2.3", "v1.2.2"],
    })).toEqual({ value: "v1.2.3", created: false });
  });

  test("starts at v0.1.0 when no release tags exist", () => {
    expect(resolveReleaseVersion({ headTags: [], allTags: [] }))
      .toEqual({ value: "v0.1.0", created: true });
  });

  test("increments the highest stable release tag", () => {
    expect(resolveReleaseVersion({
      headTags: [],
      allTags: ["v1.2.9", "v1.10.0", "draft", "v1.10.0-rc1"],
    })).toEqual({ value: "v1.10.1", created: true });
  });

  test("fails when HEAD has multiple stable release tags", () => {
    expect(() => resolveReleaseVersion({
      headTags: ["v1.0.0", "v1.0.1"],
      allTags: ["v1.0.0", "v1.0.1"],
    })).toThrow("HEAD has multiple release tags: v1.0.0 v1.0.1");
  });
});
