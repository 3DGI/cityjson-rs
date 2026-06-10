import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const root = process.cwd();
const manifestPath = path.join(root, 'src/data/api-symbols.json');
const contentDir = path.join(root, 'src/content/docs');
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const failures = [];

const entries = manifest.entries ?? [];
const symbolsByPackage = manifest.symbolsByPackage ?? {};
const urls = new Map();

for (const entry of entries) {
  if (!entry.name?.trim()) {
    failures.push('Generated symbol index contains an empty symbol name');
  }
  if (!entry.url?.startsWith('/reference/')) {
    failures.push(`${entry.package}/${entry.language}/${entry.name} has unsupported local URL ${entry.url}`);
  }
  if (urls.has(entry.url)) {
    failures.push(`Generated symbol URL is not unique: ${entry.url}`);
  }
  urls.set(entry.url, entry);
  if (entry.language === 'rust' && !entry.docsRsUrl?.startsWith('https://docs.rs/')) {
    failures.push(`${entry.package}/rust/${entry.name} is missing a docs.rs mirror URL`);
  }
}

for (const [crate, languages] of Object.entries(symbolsByPackage)) {
  for (const [language, symbols] of Object.entries(languages)) {
    for (const [symbol, href] of Object.entries(symbols)) {
      if (!symbol.trim()) {
        failures.push(`${crate}/${language} contains an empty symbol`);
      }
      if (!href.startsWith('/reference/')) {
        failures.push(`${crate}/${language}/${symbol} has unsupported href ${href}`);
      }
    }
  }
}

for (const languages of Object.values(symbolsByPackage)) {
  for (const symbols of Object.values(languages)) {
    for (const href of Object.values(symbols)) {
      const [pagePath, id] = href.replace(/^\//, '').split('#');
      const sourcePath = path.join(contentDir, `${pagePath.replace(/\/$/, '')}.mdx`);
      if (!fs.existsSync(sourcePath)) {
        failures.push(`Missing local reference page ${href}`);
        continue;
      }
      const reference = fs.readFileSync(sourcePath, 'utf8');
      if (!reference.includes(`id="${id}"`)) {
        failures.push(`Missing local reference anchor ${href}`);
      }
    }
  }
}

const codeBlockPattern = /<ApiCodeBlock\s+[^>]*crate="([^"]+)"[^>]*language="([^"]+)"/g;
for (const file of walk(contentDir)) {
  const text = fs.readFileSync(file, 'utf8');
  for (const match of text.matchAll(codeBlockPattern)) {
    const [, crate, language] = match;
    if (!symbolsByPackage[crate]) {
      failures.push(`${file} references unknown crate ${crate}`);
    } else if (!symbolsByPackage[crate][language]) {
      failures.push(`${file} references unsupported language ${language} for ${crate}`);
    }
  }

  for (const href of text.matchAll(/href="(\/reference\/[^"]+)"/g)) {
    const [pagePath, id] = href[1].replace(/^\//, '').split('#');
    const sourcePath = path.join(contentDir, `${pagePath.replace(/\/$/, '')}.mdx`);
    if (!fs.existsSync(sourcePath)) {
      failures.push(`${file} links to missing page ${href[1]}`);
      continue;
    }
    if (id && !fs.readFileSync(sourcePath, 'utf8').includes(`id="${id}"`)) {
      failures.push(`${file} links to missing anchor ${href[1]}`);
    }
  }
}

if (failures.length > 0) {
  for (const failure of failures) {
    console.error(failure);
  }
  process.exit(1);
}

function* walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      yield* walk(fullPath);
    } else if (entry.name.endsWith('.md') || entry.name.endsWith('.mdx')) {
      yield fullPath;
    }
  }
}
