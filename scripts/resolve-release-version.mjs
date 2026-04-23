import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const VERSION_PATTERN = /^v\d+\.\d+\.\d+$/;

export function parseVersionTag(tag) {
  if (!VERSION_PATTERN.test(tag)) {
    return null;
  }

  const [major, minor, patch] = tag.slice(1).split(".").map(Number);

  return { tag, major, minor, patch };
}

function compareVersionEntries(left, right) {
  if (left.major !== right.major) {
    return left.major - right.major;
  }

  if (left.minor !== right.minor) {
    return left.minor - right.minor;
  }

  return left.patch - right.patch;
}

export function findLatestReleaseTag(tags) {
  const versionEntries = tags.map(parseVersionTag).filter((entry) => entry !== null);

  if (versionEntries.length === 0) {
    return null;
  }

  versionEntries.sort(compareVersionEntries);

  return versionEntries.at(-1).tag;
}

export function incrementPatchVersion(tag) {
  const parsedTag = parseVersionTag(tag);

  if (parsedTag === null) {
    throw new Error(`Invalid version tag: ${tag}`);
  }

  return `v${parsedTag.major}.${parsedTag.minor}.${parsedTag.patch + 1}`;
}

export function resolveReleaseVersion({ headTags, allTags }) {
  const stableHeadTags = headTags.filter((tag) => VERSION_PATTERN.test(tag));

  if (stableHeadTags.length > 1) {
    throw new Error(`HEAD has multiple release tags: ${stableHeadTags.join(" ")}`);
  }

  if (stableHeadTags.length === 1) {
    return { value: stableHeadTags[0], created: false };
  }

  const latestReleaseTag = findLatestReleaseTag(allTags);

  return {
    value: latestReleaseTag === null ? "v0.1.0" : incrementPatchVersion(latestReleaseTag),
    created: true,
  };
}

function readGitLines(args) {
  return execFileSync("git", args, { encoding: "utf8" })
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

export function resolveReleaseVersionFromGit(readLines = readGitLines) {
  return resolveReleaseVersion({
    headTags: readLines(["tag", "--points-at", "HEAD"]),
    allTags: readLines(["tag", "--list", "v*"]),
  });
}

export function writeGitHubOutput(filePath, outputs) {
  const outputText = Object.entries(outputs)
    .map(([key, value]) => `${key}=${value}`)
    .join("\n");

  if (filePath) {
    appendFileSync(filePath, `${outputText}\n`);
    return;
  }

  process.stdout.write(`${outputText}\n`);
}

function isExecutedDirectly() {
  return process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isExecutedDirectly()) {
  try {
    const result = resolveReleaseVersionFromGit();

    writeGitHubOutput(process.argv[2], {
      value: result.value,
      created: String(result.created),
    });
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
