import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const IMAGE_REPOSITORY = "registry.wattly.pl/aiwattcoach";
export const VERSION_PATTERN = /^v\d+\.\d+\.\d+$/;
export const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export function printUsage(log = console.log) {
  log("Usage: bun run docker:publish:registry -- <vX.Y.Z>");
  log("Requires: docker login registry.wattly.pl");
}

export function validateVersionTag(version) {
  if (!VERSION_PATTERN.test(version)) {
    throw new Error("Version tag must match vX.Y.Z");
  }

  return version;
}

function run(command, args) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    shell: process.platform === "win32",
  });

  if (result.error) {
    throw new Error(`Failed to run ${command}: ${result.error.message}`);
  }

  if ((result.status ?? 1) !== 0) {
    throw new Error(`${command} exited with status ${result.status ?? 1}`);
  }
}

export function buildPublishCommands(version, repoRoot = REPO_ROOT) {
  const validatedVersion = validateVersionTag(version);
  const versionedImage = `${IMAGE_REPOSITORY}:${validatedVersion}`;
  const latestImage = `${IMAGE_REPOSITORY}:latest`;

  return [
    {
      command: "docker",
      args: [
        "build",
        "--platform",
        "linux/amd64",
        "-t",
        versionedImage,
        "-t",
        latestImage,
        repoRoot,
      ],
    },
    { command: "docker", args: ["push", versionedImage] },
    { command: "docker", args: ["push", latestImage] },
  ];
}

export function runPublish(
  args,
  { log = console.log, error = console.error, runCommand = run } = {},
) {
  const [version] = args;

  if (!version || version === "--help" || version === "-h") {
    printUsage(log);
    return version ? 0 : 1;
  }

  try {
    for (const command of buildPublishCommands(version)) {
      runCommand(command.command, command.args);
    }

    return 0;
  } catch (commandError) {
    error(commandError instanceof Error ? commandError.message : String(commandError));
    return 1;
  }
}

function isExecutedDirectly() {
  return process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isExecutedDirectly()) {
  process.exit(runPublish(process.argv.slice(2)));
}
