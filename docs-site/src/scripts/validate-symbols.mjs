import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const root = process.cwd();
const manifestPath = path.join(root, 'src/data/api-symbols.json');
const referencePath = path.join(root, 'src/data/api-reference.json');
const contentDir = path.join(root, 'src/content/docs');
const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const reference = JSON.parse(fs.readFileSync(referencePath, 'utf8'));
const failures = [];

const expectedGroups = new Set([
  'types',
  'standalone-functions',
  'associated-functions',
  'class-methods',
  'static-methods',
  'instance-methods',
  'constants',
  'fields',
  'symbols',
]);

const entries = manifest.entries ?? [];
const referenceEntries = reference.entries ?? [];
const symbolsByPackage = manifest.symbolsByPackage ?? {};
const urls = new Map();

for (const entry of referenceEntries) {
  if (!entry.name?.trim()) failures.push('API reference contains an empty symbol name');
  if (!entry.owner?.label || !entry.owner?.slug) {
    failures.push(`${entry.package}/${entry.language}/${entry.name} is missing owner metadata`);
  }
  if (!entry.displayKind?.trim()) {
    failures.push(`${entry.package}/${entry.language}/${entry.name} is missing displayKind metadata`);
  }
  for (const field of ['parameters', 'returns', 'raises', 'examples', 'notes', 'seeAlso']) {
    if (!Array.isArray(entry[field])) {
      failures.push(`${entry.package}/${entry.language}/${entry.name} is missing structured ${field} metadata`);
    }
  }
  if (typeof entry.summary !== 'string') {
    failures.push(`${entry.package}/${entry.language}/${entry.name} is missing summary metadata`);
  }
  if (!entry.group?.key || !entry.group?.label || typeof entry.group.order !== 'number') {
    failures.push(`${entry.package}/${entry.language}/${entry.name} is missing group metadata`);
  } else if (!expectedGroups.has(entry.group.key)) {
    failures.push(`${entry.package}/${entry.language}/${entry.name} has unknown group ${entry.group.key}`);
  }
  if (entry.language === 'cpp' && entry.name.includes('detail')) {
    failures.push(`C++ detail symbol leaked into public docs: ${entry.name}`);
  }
  if (entry.signature && !balancedSignature(entry.signature)) {
    failures.push(`${entry.package}/${entry.language}/${entry.name} has a broken or unterminated signature: ${entry.signature}`);
  }
  if (entry.kind === 'method' && !entry.owner) {
    failures.push(`${entry.package}/${entry.language}/${entry.name} method is missing an owner`);
  }
  if (entry.displayKind === 'Field' && entry.language === 'cpp' && entry.owner?.label === 'Functions') {
    failures.push(`${entry.package}/cpp/${entry.name} field is not nested under its owner page`);
  }
}

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
  if (entry.kind === 'method' && /\/method-[^/]+\//.test(entry.url)) {
    failures.push(`${entry.package}/${entry.language}/${entry.name} method is emitted as a method page instead of an owner page`);
  }
}

for (const [crate, languages] of Object.entries(symbolsByPackage)) {
  for (const [language, symbols] of Object.entries(languages)) {
    for (const [symbol, href] of Object.entries(symbols)) {
      if (!symbol.trim()) failures.push(`${crate}/${language} contains an empty symbol`);
      if (!href.startsWith('/reference/')) failures.push(`${crate}/${language}/${symbol} has unsupported href ${href}`);
    }
  }
}

for (const languages of Object.values(symbolsByPackage)) {
  for (const symbols of Object.values(languages)) {
    for (const href of Object.values(symbols)) {
      validateHref(href, 'symbol manifest');
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
    validateHref(href[1], file);
  }

  if (text.includes('\ngenerated: true\n') && text.includes('Kind: ')) {
    failures.push(`${file} still contains legacy Kind prose`);
  }
}

if (failures.length > 0) {
  for (const failure of failures) console.error(failure);
  process.exit(1);
}

function validateHref(href, source) {
  const [pagePath, id] = href.replace(/^\//, '').split('#');
  const normalized = pagePath.replace(/\/$/, '');
  const sourcePath = path.join(contentDir, `${normalized}.mdx`);
  const indexPath = path.join(contentDir, normalized, 'index.mdx');
  const resolvedPath = fs.existsSync(sourcePath) ? sourcePath : indexPath;
  if (!fs.existsSync(resolvedPath)) {
    failures.push(`${source} links to missing page ${href}`);
    return;
  }
  if (id) {
    const resolvedText = fs.readFileSync(resolvedPath, 'utf8');
    if (!resolvedText.includes(`id="${id}"`) && !resolvedText.includes(`data-api-anchor="${id}"`)) {
      failures.push(`${source} links to missing anchor ${href}`);
    }
  }
}

function balancedSignature(signature) {
  const pairs = { '(': ')', '[': ']', '{': '}' };
  const closing = new Set(Object.values(pairs));
  const stack = [];
  for (const char of signature) {
    if (pairs[char]) stack.push(pairs[char]);
    if (closing.has(char) && stack.pop() !== char) return false;
  }
  return stack.length === 0;
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
