import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  embedManifestIntoHtml,
  validateEmbeddedManifest,
  validateReleaseManifest,
} from "./release-manifest.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPOSITORY_ROOT = path.resolve(SCRIPT_DIR, "../..");

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    throw new Error(`invalid JSON in ${filePath}: ${error.message}`);
  }
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function usage() {
  console.error(
    "usage: node dev/pages/build-site.mjs build <manifest-path> [source-dir] [output-dir]",
  );
}

async function buildSite(manifestPath, sourceDir, outputDir) {
  const manifest = readJson(manifestPath);
  const manifestErrors = validateReleaseManifest(manifest);
  if (manifestErrors.length > 0) {
    throw new Error(
      `invalid release manifest ${manifestPath}:\n- ${manifestErrors.join("\n- ")}`,
    );
  }

  const sourcePath = path.resolve(REPOSITORY_ROOT, sourceDir);
  const outputPath = path.resolve(REPOSITORY_ROOT, outputDir);
  if (sourcePath === outputPath) {
    throw new Error("Pages source and output directories must be different");
  }

  await fs.promises.rm(outputPath, { recursive: true, force: true });
  await fs.promises.cp(sourcePath, outputPath, { recursive: true });

  const outputManifestPath = path.join(outputPath, "release.json");
  const outputIndexPath = path.join(outputPath, "index.html");
  writeJson(outputManifestPath, manifest);

  const sourceIndexHtml = await fs.promises.readFile(outputIndexPath, "utf8");
  const outputIndexHtml = embedManifestIntoHtml(sourceIndexHtml, manifest);
  await fs.promises.writeFile(outputIndexPath, outputIndexHtml, "utf8");

  const embeddedErrors = validateEmbeddedManifest(manifest, outputIndexHtml);
  if (embeddedErrors.length > 0) {
    throw new Error(
      `generated Pages site has invalid embedded release metadata:\n- ${embeddedErrors.join("\n- ")}`,
    );
  }

  console.log(
    `built Pages site for ${manifest.version}: ${sourcePath} -> ${outputPath}`,
  );
}

async function main() {
  const [command, manifestPath, sourceDir = "docs", outputDir = "dist/pages"] =
    process.argv.slice(2);
  if (command !== "build" || !manifestPath) {
    usage();
    process.exitCode = 2;
    return;
  }

  await buildSite(manifestPath, sourceDir, outputDir);
}

main().catch((error) => {
  console.error(`error: ${error.message}`);
  process.exitCode = 1;
});
