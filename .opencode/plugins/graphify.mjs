import { existsSync } from "fs";
import { join } from "path";

export const id = "graphify";

function buildGraphifyReminder(directory) {
  const graphReportPath = join(directory, "graphify-out", "GRAPH_REPORT.md");

  if (!existsSync(graphReportPath)) {
    return null;
  }

  const wikiIndexPath = join(directory, "graphify-out", "wiki", "index.md");
  const graphJsonPath = join(directory, "graphify-out", "graph.json");
  const reminder = [
    "Graphify knowledge graph is available in `graphify-out/`.",
    "Before answering architecture or codebase questions, read `graphify-out/GRAPH_REPORT.md`.",
    existsSync(wikiIndexPath)
      ? "If `graphify-out/wiki/index.md` exists, use it as the main navigation entry instead of exploring raw graph artifacts first."
      : null,
    existsSync(graphJsonPath)
      ? "Treat the graph as navigation aid, then confirm behavior in real source files before changing code."
      : null,
    "After modifying code files in this repo, run `./scripts/rebuild_graphify.sh`.",
  ]
    .filter(Boolean)
    .join(" ");

  return reminder;
}

export const server = async ({ directory }) => {
  const reminder = buildGraphifyReminder(directory);

  if (!reminder) {
    return {};
  }

  return {
    "experimental.chat.system.transform": async (_input, output) => {
      output.system.push(reminder);
    },
  };
};
