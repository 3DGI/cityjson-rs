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
const groupOrder = [
  'types',
  'standalone-functions',
  'associated-functions',
  'class-methods',
  'static-methods',
  'instance-methods',
  'constants',
  'fields',
  'symbols',
];

if (!fs.existsSync(referenceDataPath)) {
  throw new Error(`Missing ${referenceDataPath}; run npm run extract:api first`);
}

const reference = JSON.parse(fs.readFileSync(referenceDataPath, 'utf8'));
const entries = assignUrls((reference.entries ?? []).map(normalizeEntry));

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

function normalizeEntry(entry) {
  const normalized = { ...entry };
  normalized.group = normalizeGroup(entry.group, entry.kind);
  normalized.displayKind = entry.displayKind ?? fallbackDisplayKind(entry.kind);
  return normalized;
}

function normalizeGroup(group, kind) {
  if (group?.key && group?.label && typeof group.order === 'number') {
    return group;
  }

  const fallbackGroups = {
    class: { key: 'types', label: 'Types', order: 10 },
    struct: { key: 'types', label: 'Types', order: 10 },
    enum: { key: 'types', label: 'Types', order: 10 },
    'type alias': { key: 'types', label: 'Types', order: 10 },
    function: { key: 'standalone-functions', label: 'Standalone functions', order: 20 },
    method: { key: 'instance-methods', label: 'Instance methods', order: 60 },
    constant: { key: 'constants', label: 'Constants', order: 70 },
  };

  return fallbackGroups[kind] ?? { key: 'symbols', label: 'Symbols', order: 90 };
}

function fallbackDisplayKind(kind) {
  const labels = {
    class: 'Class',
    struct: 'Struct',
    enum: 'Enum',
    'type alias': 'Type alias',
    function: 'Standalone function',
    method: 'Method',
    constant: 'Constant',
  };
  return labels[kind] ?? kind;
}

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
      return `- [${escapeMd(owner.label)}](./${owner.slug}/) - ${summarizeOwner(ownerEntries)}`;
    })
    .join('\n');

  fs.writeFileSync(
    path.join(packageDir, 'index.mdx'),
    `---\ntitle: ${title}\ndescription: Generated ${languageLabels[first.language]} API reference for ${first.package}.\nlanguages: ["${first.language}"]\ngenerated: true\n---\n\nThis generated index links to owner-level reference pages for ${languageLabels[first.language]} symbols.\n\n${owners}\n`,
    'utf8',
  );
}

function summarizeOwner(ownerEntries) {
  const parts = groupedEntries(ownerEntries).map((entries) => {
    const count = entries.length;
    const label = entries[0].group.label.toLowerCase();
    const noun = count === 1 ? label.slice(0, -1) : label;
    return `${count} ${noun}`;
  });
  return parts.join(', ');
}

function renderOwnerPage(ownerEntries) {
  const first = ownerEntries[0];
  const owner = first.owner;
  const packageDir = path.join(referenceDir, first.package, first.language, owner.slug);
  fs.mkdirSync(packageDir, { recursive: true });

  const groups = groupedEntries(ownerEntries);
  const sourceDetail = first.source.detail ? `\nSource metadata: ${escapeMdxText(first.source.detail)}\n` : '';
  const summaries = groups.map((entries) => renderSummaryTable(entries)).join('\n\n');
  const sections = groups.map((entries) => renderGroupSection(entries)).join('\n\n');

  fs.writeFileSync(
    path.join(packageDir, 'index.mdx'),
    `---\ntitle: ${owner.label}\ndescription: Generated ${languageLabels[first.language]} owner reference for ${first.package}.\nlanguages: ["${first.language}"]\ngenerated: true\n---\n\n<div class="api-owner-source">\nSource: <code>${escapeMdxText(first.source.path)}</code>${sourceDetail}\n</div>\n\n## Summary\n\n${summaries}\n\n${sections}\n`,
    'utf8',
  );
}

function groupedEntries(entries) {
  return [...groupBy([...entries].sort(ownerEntrySort), (entry) => entry.group.key).values()].sort((a, b) =>
    groupSort(a[0], b[0]),
  );
}

function ownerEntrySort(a, b) {
  return groupSort(a, b) || kindSort(a, b) || a.memberName.localeCompare(b.memberName);
}

function groupSort(a, b) {
  const aIndex = groupOrder.indexOf(a.group.key);
  const bIndex = groupOrder.indexOf(b.group.key);
  const normalizedA = aIndex === -1 ? Number(a.group.order ?? 999) : aIndex;
  const normalizedB = bIndex === -1 ? Number(b.group.order ?? 999) : bIndex;
  if (normalizedA !== normalizedB) return normalizedA - normalizedB;
  return String(a.group.label).localeCompare(String(b.group.label));
}

function renderSummaryTable(entries) {
  const group = entries[0].group;
  const showClassification = shouldShowClassification(entries);
  const headers = showClassification
    ? '<th>Name</th><th>Classification</th><th>Signature</th><th>Summary</th>'
    : '<th>Name</th><th>Signature</th><th>Summary</th>';
  const rows = entries
    .map((entry) => {
      const summary = entry.summary || firstSentence(entry.docs) || '';
      const classification = showClassification
        ? `
<td><span class="api-kind-label">${escapeMdxText(entry.displayKind)}</span></td>`
        : '';
      return `<tr>
<td><a href="#${entry.anchor}"><code>${escapeMdxText(entry.memberName)}</code></a></td>${classification}
<td><code>${escapeMdxText(compactSignature(entry.signature))}</code></td>
<td>${renderInlineMarkdown(summary)}</td>
</tr>`;
    })
    .join('\n');
  return `<section class="api-summary-section">

<div class="api-summary-group-label">${escapeMdxText(group.label)}</div>

<table class="api-summary-table">
<thead><tr>${headers}</tr></thead>
<tbody>
${rows}
</tbody>
</table>

</section>`;
}

function shouldShowClassification(entries) {
  const group = entries[0].group;
  if (group.key === 'types' || group.key === 'symbols') return true;
  const firstKind = entries[0].displayKind;
  return entries.some((entry) => entry.displayKind !== firstKind);
}

function renderGroupSection(entries) {
  const group = entries[0].group;
  const symbols = entries.map((entry) => renderSymbolSection(entry)).join('\n\n');
  return `## ${escapeMdxText(group.label)}

<div class="api-reference-group">

${symbols}

</div>`;
}

function renderSymbolSection(entry) {
  const signature = entry.signature
    ? `

<div class="api-signature">

\`\`\`${fenceLanguage(entry.language)}
${entry.signature}
\`\`\`

</div>`
    : '';
  const summary = entry.summary ? `

${entry.summary}` : '';
  const structured = renderStructuredSections(entry);
  const fallbackDocs = !entry.summary && !structured && entry.docs ? `

${entry.docs}` : '';
  const mirror = entry.docsRsUrl ? `

[docs.rs mirror](${entry.docsRsUrl})` : '';
  return `<section class="api-reference-symbol" data-pagefind-body aria-labelledby="${entry.anchor}">

### <span data-api-anchor="${entry.anchor}"></span>\`${escapeMd(entry.memberName)}\`

${signature}${summary}${structured}${fallbackDocs}${mirror}

</section>`;
}

function renderStructuredSections(entry) {
  const sections = [];
  if (entry.parameters?.length) sections.push(renderDefinitionSection('Parameters', entry.parameters, 'name'));
  if (entry.returns?.length) sections.push(renderDefinitionSection('Returns', entry.returns, 'type'));
  if (entry.raises?.length) sections.push(renderDefinitionSection('Raises', entry.raises, 'type'));
  if (entry.notes?.length) sections.push(renderTextListSection('Notes', entry.notes));
  if (entry.seeAlso?.length) sections.push(renderTextListSection('See Also', entry.seeAlso));
  if (entry.examples?.length) sections.push(renderTextListSection('Examples', entry.examples));
  return sections.length ? `

${sections.join('\n\n')}` : '';
}

function renderDefinitionSection(title, items, termKey) {
  const terms = items
    .map((item) => {
      const term = plainDefinitionTerm(item[termKey] || item.name || item.type || 'value');
      const type = termKey === 'name' && item.type ? ` <span class="api-param-type">${renderInlineMarkdown(item.type)}</span>` : '';
      const description = item.description ? renderInlineMarkdown(item.description) : '';
      return `<dt><code>${escapeMdxText(term)}</code>${type}</dt><dd>${description}</dd>`;
    })
    .join('\n');
  return `#### ${title}

<dl class="api-definition-list">
${terms}
</dl>`;
}

function renderTextListSection(title, values) {
  return `#### ${title}

${values.join('\n\n')}`;
}

function symbolManifestEntry(entry) {

  return {
    name: entry.name,
    memberName: entry.memberName,
    owner: entry.owner.label,
    package: entry.package,
    language: entry.language,
    kind: entry.kind,
    displayKind: entry.displayKind,
    group: entry.group,
    signature: entry.signature,
    source: entry.source.path,
    docsRsUrl: entry.docsRsUrl,
    url: entry.url,
    aliases: entry.aliases,
    summary: entry.summary,
    parameters: entry.parameters,
    returns: entry.returns,
    raises: entry.raises,
    examples: entry.examples,
    notes: entry.notes,
    seeAlso: entry.seeAlso,
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


function plainDefinitionTerm(value) {
  return String(value ?? '').replace(/^`|`$/g, '').trim();
}

function firstSentence(value) {
  const text = String(value ?? '').replace(/\s+/g, ' ').trim();
  if (!text) return '';
  const match = text.match(/^(.+?[.!?])\s/);
  return match ? match[1] : text;
}

function compactSignature(value) {
  const signature = String(value ?? '').replace(/\s+/g, ' ').trim();
  return signature.length > 96 ? `${signature.slice(0, 93)}...` : signature;
}

function renderInlineMarkdown(value) {
  return escapeMdxText(String(value ?? ''))
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
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
