import { spawnSync } from "node:child_process";

const IMAGE_REPOSITORY = "registry.wattly.pl/aiwattcoach";
const VERSION_PATTERN = /^v\d+\.\d+\.\d+$/;

function printUsage() {
  console.log("Usage: bun run docker:publish:registry -- <vX.Y.Z>");
  console.log("Requires: docker login registry.wattly.pl");
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

function run(command, args) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    shell: process.platform === "win32",
  });

  if (result.error) {
    fail(`Failed to run ${command}: ${result.error.message}`);
  }

  if ((result.status ?? 1) !== 0) {
    process.exit(result.status ?? 1);
  }
}

const [version] = process.argv.slice(2);

if (!version || version === "--help" || version === "-h") {
  printUsage();
  process.exit(version ? 0 : 1);
}

if (!VERSION_PATTERN.test(version)) {
  fail("Version tag must match vX.Y.Z");
}

const versionedImage = `${IMAGE_REPOSITORY}:${version}`;
const latestImage = `${IMAGE_REPOSITORY}:latest`;

run("docker", [
  "build",
  "--platform",
  "linux/amd64",
  "-t",
  versionedImage,
  "-t",
  latestImage,
  ".",
]);
run("docker", ["push", versionedImage]);
run("docker", ["push", latestImage]);
