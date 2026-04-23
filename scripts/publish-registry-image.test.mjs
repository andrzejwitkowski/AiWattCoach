import { describe, expect, test } from "bun:test";

import {
  REPO_ROOT,
  buildPublishCommands,
  runPublish,
} from "./publish-registry-image.mjs";

describe("buildPublishCommands", () => {
  test("builds docker commands with version and latest tags", () => {
    expect(buildPublishCommands("v1.2.3")).toEqual([
      {
        command: "docker",
        args: [
          "build",
          "--platform",
          "linux/amd64",
          "-t",
          "registry.wattly.pl/aiwattcoach:v1.2.3",
          "-t",
          "registry.wattly.pl/aiwattcoach:latest",
          REPO_ROOT,
        ],
      },
      {
        command: "docker",
        args: ["push", "registry.wattly.pl/aiwattcoach:v1.2.3"],
      },
      {
        command: "docker",
        args: ["push", "registry.wattly.pl/aiwattcoach:latest"],
      },
    ]);
  });

  test("rejects invalid tags", () => {
    expect(() => buildPublishCommands("1.2.3")).toThrow("Version tag must match vX.Y.Z");
  });
});

describe("runPublish", () => {
  test("prints usage and fails when version is missing", () => {
    const output = [];

    const exitCode = runPublish([], {
      log: (line) => output.push(line),
      error: () => {},
      runCommand: () => {
        throw new Error("should not run");
      },
    });

    expect(exitCode).toBe(1);
    expect(output).toEqual([
      "Usage: bun run docker:publish:registry -- <vX.Y.Z>",
      "Requires: docker login registry.wattly.pl",
    ]);
  });

  test("prints usage and succeeds for --help", () => {
    const output = [];

    const exitCode = runPublish(["--help"], {
      log: (line) => output.push(line),
      error: () => {},
      runCommand: () => {
        throw new Error("should not run");
      },
    });

    expect(exitCode).toBe(0);
    expect(output).toHaveLength(2);
  });

  test("runs docker build and push commands in order", () => {
    const calls = [];

    const exitCode = runPublish(["v2.3.4"], {
      log: () => {},
      error: () => {},
      runCommand: (command, args) => calls.push({ command, args }),
    });

    expect(exitCode).toBe(0);
    expect(calls).toEqual(buildPublishCommands("v2.3.4"));
  });
});
