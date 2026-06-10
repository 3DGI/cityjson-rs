import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const docsRoot = process.cwd();
const dataDir = path.join(docsRoot, 'src/data');
const referenceDataPath = path.join(dataDir, 'api-reference.json');
const symbolsPath = path.join(dataDir, 'api-symbols.json');
const referenceDir = path.join(docsRoot, 'src/content/docs/reference');

const languageLabels = {
  rust: 'Rust',
  python: 'Python',
  cpp: 'C++',
  wasm: 'WASM',
  c: 'C FFI',
};

const kindOrder = ['class', 'struct', 'enum', 'type alias', 'constant', 'function', 'method'];

if (!fs.existsSync(referenceDataPath)) {
  throw new Error(`Missing ${referenceDataPath}; run npm run extract:api first`);
}

const reference = JSON.parse(fs.readFileSync(referenceDataPath, 'utf8'));
const entries = assignUrls(reference.entries ?? []);

removeGeneratedPages(referenceDir);

const allSymbols = [];
const symbolsByPackage = {};
for (const group of groupBy(entries, (entry) => `${entry.package}\0${entry.language}`).values()) {
  renderLanguageIndex(group);
  for (const ownerEntries of groupBy(group, (entry) => entry.owner.key).values()) {
    renderOwnerPage(ownerEntries);
  }

  for (const entry of group) {
    allSymbols.push(symbolManifestEntry(entry));
    symbolsByPackage[entry.package] ??= {};
    symbolsByPackage[entry.package][entry.language] ??= {};
    for (const alias of entry.aliases) {
      symbolsByPackage[entry.package][entry.language][alias] = entry.url;
    }
  }
}

allSymbols.sort((a, b) =>
  [a.package, a.language, a.name].join('\0').localeCompare([b.package, b.language, b.name].join('\0')),
);

fs.writeFileSync(
  symbolsPath,
  `${JSON.stringify({ entries: allSymbols, symbolsByPackage }, null, 2)}\n`,
  'utf8',
);

function assignUrls(values) {
  const ownerSlugs = new Map();
  const ownerSlugSets = new Map();

  for (const group of groupBy(values, (entry) => `${entry.package}\0${entry.language}`).values()) {
    const first = group[0];
    const slugSetKey = `${first.package}\0${first.language}`;
    const seen = ownerSlugSets.get(slugSetKey) ?? new Set();
    ownerSlugSets.set(slugSetKey, seen);

    for (const ownerEntries of groupBy(group, (entry) => entry.owner.key).values()) {
      const owner = ownerEntries[0].owner;
      ownerSlugs.set(ownerKey(ownerEntries[0]), uniqueSlug(owner.slug, seen));
    }
  }

  const anchorsByPage = new Map();
  return values.map((entry) => {
    const pageSlug = ownerSlugs.get(ownerKey(entry));
    const pageKey = `${entry.package}\0${entry.language}\0${pageSlug}`;
    const anchors = anchorsByPage.get(pageKey) ?? new Set();
    const anchor = uniqueSlug(slugify(`${entry.kind}-${entry.memberName}`), anchors);
    anchorsByPage.set(pageKey, anchors);
    return {
      ...entry,
      anchor,
      url: `/reference/${entry.package}/${entry.language}/${pageSlug}/#${anchor}`,
      owner: { ...entry.owner, slug: pageSlug },
    };
  });
}

function renderLanguageIndex(group) {
  const first = group[0];
  const title = `${first.package} ${languageLabels[first.language]} API`;
  const packageDir = path.join(referenceDir, first.package, first.language);
  fs.mkdirSync(packageDir, { recursive: true });

  const owners = [...groupBy(group, (entry) => entry.owner.key).values()]
    .map((ownerEntries) => {
      const owner = ownerEntries[0].owner;
      const memberCount = ownerEntries.length;
      const noun = memberCount === 1 ? 'symbol' : 'symbols';
      return `- [${escapeMd(owner.label)}](./${owner.slug}/) - ${memberCount} ${noun}`;
    })
    .join('\n');

  fs.writeFileSync(
    path.join(packageDir, 'index.mdx'),
    `---\ntitle: ${title}\ndescription: Generated ${languageLabels[first.language]} API reference for ${first.package}.\nlanguages: ["${first.language}"]\ngenerated: true\n---\n\nThis generated index links to owner-level reference pages for ${languageLabels[first.language]} symbols.\n\n${owners}\n`,
    'utf8',
  );
}

function renderOwnerPage(ownerEntries) {
  const first = ownerEntries[0];
  const owner = first.owner;
  const packageDir = path.join(referenceDir, first.package, first.language, owner.slug);
  fs.mkdirSync(packageDir, { recursive: true });

  const ownedEntries = ownerEntries.toSorted((a, b) => kindSort(a, b) || a.memberName.localeCompare(b.memberName));
  const toc = ownedEntries
    .map((entry) => `- [${escapeMd(entry.memberName)}](#${entry.anchor})`)
    .join('\n');
  const memberTable = ownedEntries
    .map((entry) => `| \`${escapeMd(entry.memberName)}\` | ${entry.kind} | [section](#${entry.anchor}) |`)
    .join('\n');
  const sections = ownedEntries.map((entry) => renderSymbolSection(entry)).join('\n\n');

  fs.writeFileSync(
    path.join(packageDir, 'index.mdx'),
    `---\ntitle: ${owner.label}\ndescription: Generated ${languageLabels[first.language]} owner reference for ${first.package}.\nlanguages: ["${first.language}"]\ngenerated: true\n---\n\nSource: \`${escapeMd(first.source.path)}\`\n\n## Contents\n\n${toc}\n\n## Members\n\n| Symbol | Kind | Reference |\n| --- | --- | --- |\n${memberTable}\n\n${sections}\n`,
    'utf8',
  );
}

function renderSymbolSection(entry) {
  const signature = entry.signature
    ? `\n\n\`\`\`${fenceLanguage(entry.language)}\n${entry.signature}\n\`\`\``
    : '';
  const docs = entry.docs ? `\n\n${entry.docs}` : '';
  const source = entry.source.detail ? `\n\nSource metadata: ${escapeMdxText(entry.source.detail)}` : '';
  const mirror = entry.docsRsUrl ? `\n\n[docs.rs mirror](${entry.docsRsUrl})` : '';
  return `<section id="${entry.anchor}" class="api-reference-symbol" data-pagefind-body>\n\n### \`${escapeMd(entry.memberName)}\`\n\nKind: ${entry.kind}.${signature}${docs}${source}${mirror}\n\n</section>`;
}

function symbolManifestEntry(entry) {
  return {
    name: entry.name,
    memberName: entry.memberName,
    owner: entry.owner.label,
    package: entry.package,
    language: entry.language,
    kind: entry.kind,
    signature: entry.signature,
    source: entry.source.path,
    docsRsUrl: entry.docsRsUrl,
    url: entry.url,
    aliases: entry.aliases,
  };
}

function removeGeneratedPages(dir) {
  if (!fs.existsSync(dir)) return;
  for (const file of walk(dir)) {
    if (!file.endsWith('.mdx')) continue;
    const text = fs.readFileSync(file, 'utf8');
    if (text.includes('\ngenerated: true\n')) {
      fs.unlinkSync(file);
    }
  }
  pruneEmptyDirs(dir);
}

function* walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      yield* walk(fullPath);
    } else {
      yield fullPath;
    }
  }
}

function pruneEmptyDirs(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const fullPath = path.join(dir, entry.name);
    pruneEmptyDirs(fullPath);
    if (fs.readdirSync(fullPath).length === 0) fs.rmdirSync(fullPath);
  }
}

function ownerKey(entry) {
  return `${entry.package}\0${entry.language}\0${entry.owner.key}`;
}

function uniqueSlug(base, seen) {
  const fallback = base || 'symbols';
  let slug = fallback;
  let suffix = 2;
  while (seen.has(slug)) {
    slug = `${fallback}-${suffix}`;
    suffix += 1;
  }
  seen.add(slug);
  return slug;
}

function kindSort(a, b) {
  return kindOrder.indexOf(a.kind) - kindOrder.indexOf(b.kind);
}

function groupBy(values, keyFn) {
  const groups = new Map();
  for (const value of values) {
    const key = keyFn(value);
    groups.set(key, [...(groups.get(key) ?? []), value]);
  }
  return groups;
}

function slugify(value) {
  return value
    .replace(/::/g, '-')
    .replace(/\./g, '-')
    .replace(/[^A-Za-z0-9_-]+/g, '-')
    .replace(/^-|-$/g, '')
    .toLowerCase();
}

function fenceLanguage(language) {
  if (language === 'cpp') return 'cpp';
  if (language === 'c') return 'c';
  if (language === 'wasm') return 'rust';
  return language;
}

function escapeMd(value) {
  return String(value).replaceAll('`', '\\`');
}

function escapeMdxText(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('{', '&#123;')
    .replaceAll('}', '&#125;');
}
