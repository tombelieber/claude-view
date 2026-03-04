#!/usr/bin/env node

import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

const rootDir = process.cwd();

const IGNORE_DIRS = new Set([
  ".git",
  ".turbo",
  ".tmp",
  ".wrangler",
  "node_modules",
  "target",
  "dist",
  "build",
  ".next",
  ".idea",
  ".vscode",
]);

function findPackageJsonFiles(startDir) {
  const files = [];
  const stack = [startDir];

  while (stack.length > 0) {
    const dir = stack.pop();
    if (!dir) continue;

    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!IGNORE_DIRS.has(entry.name)) {
          stack.push(path.join(dir, entry.name));
        }
        continue;
      }

      if (entry.isFile() && entry.name === "package.json") {
        files.push(path.join(dir, entry.name));
      }
    }
  }

  return files.sort((a, b) => a.localeCompare(b));
}

function relativePath(file) {
  return path.relative(rootDir, file) || file;
}

let hasError = false;
const manifestFiles = findPackageJsonFiles(rootDir);

if (manifestFiles.length === 0) {
  console.error("No package.json files found.");
  process.exit(1);
}

for (const file of manifestFiles) {
  try {
    JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    hasError = true;
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Invalid JSON in ${relativePath(file)}\n${message}\n`);
  }
}

if (hasError) {
  process.exit(1);
}

console.log(`Verified ${manifestFiles.length} package.json files.`);
