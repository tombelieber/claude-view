// Concatenates all MDX/MD content into public/llms-full.txt for AI agent consumption
import { readdir, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const contentDir = resolve(__dirname, '../src/content');
const outputPath = resolve(__dirname, '../public/llms-full.txt');

if (!existsSync(contentDir)) {
  console.warn('Content directory not found, skipping llms-full.txt generation');
  process.exit(0);
}

async function collectFiles(dir, base = dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...await collectFiles(fullPath, base));
    } else if (/\.(md|mdx)$/.test(entry.name)) {
      files.push(fullPath);
    }
  }
  return files;
}

const files = (await collectFiles(contentDir)).sort();
const sections = [];
for (const file of files) {
  const content = await readFile(file, 'utf-8');
  const relative = file.replace(contentDir + '/', '');
  sections.push(`--- ${relative} ---\n\n${content}`);
}

await writeFile(outputPath, sections.join('\n\n'));
console.log(`llms-full.txt generated (${files.length} files)`);
