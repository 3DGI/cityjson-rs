import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const docsRoot = process.cwd();
const repoRoot = path.resolve(docsRoot, '..');
const dataPath = path.join(docsRoot, 'src/data/api-symbols.json');
const referenceDir = path.join(docsRoot, 'src/content/docs/reference');
const generatedPages = [
  'cityjson-lib/rust.mdx',
  'cityjson-lib/python.mdx',
  'cityjson-lib/cpp.mdx',
  'cityjson-lib/wasm.mdx',
  'cityjson-index/rust.mdx',
  'cityjson-index/python.mdx',
];

const languages = {
  rust: 'Rust',
  python: 'Python',
  cpp: 'C++',
  wasm: 'WASM',
};

const targets = [
  {
    package: 'cityjson-lib',
    title: 'cityjson-lib Rust API',
    language: 'rust',
    source: 'crates/cityjson-lib/src',
    docsRsBase: 'https://docs.rs/cityjson-lib/latest/cityjson_lib/',
    entries: rustCityjsonLib,
  },
  {
    package: 'cityjson-lib',
    title: 'cityjson-lib Python API',
    language: 'python',
    source: 'crates/cityjson-lib/ffi/python/src/cityjson_lib/__init__.py',
    entries: pythonApi,
  },
  {
    package: 'cityjson-lib',
    title: 'cityjson-lib C++ API',
    language: 'cpp',
    source: 'crates/cityjson-lib/ffi/cpp/include/cityjson_lib/cityjson_lib.hpp',
    entries: cppApi,
  },
  {
    package: 'cityjson-lib',
    title: 'cityjson-lib WASM API',
    language: 'wasm',
    source: 'crates/cityjson-lib/ffi/wasm/src/lib.rs',
    entries: wasmApi,
  },
  {
    package: 'cityjson-index',
    title: 'cityjson-index Rust API',
    language: 'rust',
    source: 'crates/cityjson-index/src/lib.rs',
    docsRsBase: 'https://docs.rs/cityjson-index/latest/cityjson_index/',
    entries: rustCityjsonIndex,
  },
  {
    package: 'cityjson-index',
    title: 'cityjson-index Python API',
    language: 'python',
    source: 'crates/cityjson-index/ffi/python/src/cityjson_index/__init__.py',
    entries: pythonApi,
  },
];

const allEntries = [];
const symbolsByPackage = {};

fs.mkdirSync(referenceDir, { recursive: true });
for (const page of generatedPages) {
  const fullPath = path.join(referenceDir, page);
  if (fs.existsSync(fullPath)) fs.unlinkSync(fullPath);
}

for (const target of targets) {
  const sourcePath = path.join(repoRoot, target.source);
  const source = readSource(sourcePath);
  const entries = target
    .entries(source, target)
    .filter((entry) => !entry.name.startsWith('_'))
    .map((entry) => normalizeEntry(entry, target));
  const uniqueEntries = assignUniqueAnchors(uniqueBy(entries, (entry) => `${entry.kind}:${entry.name}`));
  const slug = `/reference/${target.package}/${target.language}/`;
  const pagePath = path.join(referenceDir, target.package, `${target.language}.mdx`);

  fs.mkdirSync(path.dirname(pagePath), { recursive: true });
  fs.writeFileSync(pagePath, renderReferencePage(target, uniqueEntries), 'utf8');

  for (const entry of uniqueEntries) {
    const url = `${slug}#${entry.anchor}`;
    const enriched = { ...entry, url };
    delete enriched.anchor;
    allEntries.push(enriched);
    symbolsByPackage[target.package] ??= {};
    symbolsByPackage[target.package][target.language] ??= {};
    for (const alias of entry.aliases) {
      symbolsByPackage[target.package][target.language][alias] = url;
    }
  }
}

allEntries.sort((a, b) =>
  [a.package, a.language, a.name].join('\0').localeCompare([b.package, b.language, b.name].join('\0')),
);

fs.writeFileSync(
  dataPath,
  `${JSON.stringify({ entries: allEntries, symbolsByPackage }, null, 2)}\n`,
  'utf8',
);

function readSource(sourcePath) {
  if (fs.statSync(sourcePath).isDirectory()) {
    return fs
      .readdirSync(sourcePath)
      .filter((file) => file.endsWith('.rs'))
      .map((file) => `// ${file}\n${fs.readFileSync(path.join(sourcePath, file), 'utf8')}`)
      .join('\n');
  }
  return fs.readFileSync(sourcePath, 'utf8');
}

function normalizeEntry(entry, target) {
  const name = entry.name.trim();
  const anchor = slugify(`${entry.kind}-${name}`);
  const aliases = uniqueBy([name, ...(entry.aliases ?? [])].filter(Boolean), (alias) => alias);
  return {
    name,
    language: target.language,
    package: target.package,
    kind: entry.kind,
    signature: entry.signature?.trim(),
    source: target.source,
    docsRsUrl: target.docsRsBase ? rustDocsRsUrl(target, name, entry.kind) : undefined,
    anchor,
    aliases,
  };
}

function renderReferencePage(target, entries) {
  const grouped = groupBy(entries, (entry) => entry.kind);
  const sourceLabel = target.source.replaceAll('\\', '/');
  const sections = ['class', 'struct', 'enum', 'type alias', 'constant', 'function', 'method']
    .filter((kind) => grouped.has(kind))
    .map((kind) => {
      const items = grouped
        .get(kind)
        .map((entry) => {
          const docsRs = entry.docsRsUrl ? `\n\n[docs.rs mirror](${entry.docsRsUrl})` : '';
          const signature = entry.signature
            ? `\n\n\`\`\`${fenceLanguage(target.language)}\n${entry.signature}\n\`\`\``
            : '';
          return `<section id="${entry.anchor}" class="api-reference-symbol" data-pagefind-body>\n\n### \`${escapeMd(entry.name)}\`\n\nKind: ${kind}.${signature}${docsRs}\n\n</section>`;
        })
        .join('\n\n');
      return `## ${kindLabel(kind)}\n\n${items}`;
    })
    .join('\n\n');

  return `---\ntitle: ${target.title}\ndescription: Generated public ${languages[target.language]} API reference for ${target.package}.\nlanguages: ["${target.language}"]\n---\n\nThis generated page indexes public user-facing ${languages[target.language]} symbols from \`${sourceLabel}\`.\n\n${sections}\n`;
}

function rustCityjsonLib(_source, target) {
  const entries = [];
  const root = readSource(path.join(repoRoot, 'crates/cityjson-lib/src/lib.rs'));
  for (const match of root.matchAll(/^pub use (?<path>[^;]+);/gm)) {
    const rawPath = match.groups.path.trim();
    const name = rawPath.includes(' as ')
      ? rawPath.split(/\s+as\s+/).at(-1)
      : rawPath.split('::').at(-1);
    if (!name || name === 'cityjson_types') continue;
    entries.push({ name: `cityjson_lib::${name}`, kind: 'type alias', aliases: [name] });
  }
  for (const moduleName of ['json', 'arrow', 'parquet', 'ops']) {
    const modulePath = path.join(repoRoot, `crates/cityjson-lib/src/${moduleName}.rs`);
    if (!fs.existsSync(modulePath)) continue;
    const moduleSource = fs.readFileSync(modulePath, 'utf8');
    for (const entry of rustItems(moduleSource, moduleName, target)) entries.push(entry);
  }
  entries.push({
    name: 'cityjson_lib::query::summary',
    kind: 'function',
    aliases: ['query::summary', 'summary'],
    signature: 'pub fn summary(model: &CityModel) -> ModelSummary',
  });
  return entries;
}

function rustCityjsonIndex(source, target) {
  return rustItems(source, '', target).filter(
    (entry) =>
      !entry.name.includes('benchmark::') &&
      !entry.name.includes('profile::') &&
      !entry.name.includes('PackageFilterDiagnostics'),
  );
}

function rustItems(source, moduleName, target) {
  const entries = [];
  const crateName = target.package.replace('-', '_');
  const prefix = moduleName ? `${crateName}::${moduleName}::` : `${crateName}::`;

  for (const match of source.matchAll(/^pub\s+(struct|enum|const|type)\s+([A-Za-z][A-Za-z0-9_]*)/gm)) {
    const [, rawKind, name] = match;
    entries.push({
      name: `${prefix}${name}`,
      kind: rawKind === 'const' ? 'constant' : rawKind === 'type' ? 'type alias' : rawKind,
      aliases: [moduleName ? `${moduleName}::${name}` : name],
      signature: firstLineAt(source, match.index),
    });
  }

  for (const match of source.matchAll(/^pub\s+fn\s+([A-Za-z][A-Za-z0-9_]*)/gm)) {
    const name = match[1];
    entries.push({
      name: `${prefix}${name}`,
      kind: 'function',
      aliases: [moduleName ? `${moduleName}::${name}` : name],
      signature: compactSignature(source, match.index),
    });
  }

  for (const implMatch of source.matchAll(/^impl\s+([A-Za-z][A-Za-z0-9_]*)\s*\{/gm)) {
    const typeName = implMatch[1];
    const bodyStart = source.indexOf('{', implMatch.index) + 1;
    const bodyEnd = matchingBrace(source, bodyStart - 1);
    const body = source.slice(bodyStart, bodyEnd);
    for (const method of body.matchAll(/^\s+pub\s+fn\s+([A-Za-z][A-Za-z0-9_]*)/gm)) {
      const methodName = method[1];
      const absoluteIndex = bodyStart + method.index;
      entries.push({
        name: `${prefix}${typeName}::${methodName}`,
        kind: 'method',
        aliases: [`${typeName}::${methodName}`, methodName],
        signature: compactSignature(source, absoluteIndex),
      });
    }
  }

  return entries;
}

function pythonApi(source) {
  const exported = new Set([...source.matchAll(/__all__\s*=\s*\[([\s\S]*?)\]/g)].flatMap((match) => [...match[1].matchAll(/"([^"]+)"/g)].map((item) => item[1])));
  const entries = [];
  const classes = [...source.matchAll(/^class\s+([A-Za-z][A-Za-z0-9_]*)[(:]/gm)];
  for (const match of classes) {
    const className = match[1];
    if (!exported.has(className)) continue;
    const bodyStart = source.indexOf('\n', match.index) + 1;
    const bodyEnd = nextTopLevel(source, bodyStart);
    const body = source.slice(bodyStart, bodyEnd);
    entries.push({ name: className, kind: 'class', signature: firstLineAt(source, match.index) });
    for (const method of body.matchAll(/^    def\s+([A-Za-z][A-Za-z0-9_]*)\(/gm)) {
      const methodName = method[1];
      if (methodName.startsWith('_') || ['to_native', 'from_native', 'to_native_payload'].includes(methodName)) continue;
      entries.push({
        name: `${className}.${methodName}`,
        kind: 'method',
        aliases: [methodName],
        signature: compactPythonSignature(source, bodyStart + method.index),
      });
    }
    for (const method of body.matchAll(/^    @classmethod\n    def\s+([A-Za-z][A-Za-z0-9_]*)\(/gm)) {
      const methodName = method[1];
      if (methodName.startsWith('_') || ['to_native', 'from_native', 'to_native_payload'].includes(methodName)) continue;
      entries.push({
        name: `${className}.${methodName}`,
        kind: 'method',
        aliases: [methodName],
        signature: compactPythonSignature(source, bodyStart + method.index + method[0].indexOf('def')),
      });
    }
  }

  for (const match of source.matchAll(/^def\s+([A-Za-z][A-Za-z0-9_]*)\(/gm)) {
    const name = match[1];
    if (!exported.has(name) || name.startsWith('_')) continue;
    entries.push({ name, kind: 'function', signature: compactPythonSignature(source, match.index) });
  }
  return entries;
}

function cppApi(source) {
  const publicSource = source.replace(/namespace detail \{[\s\S]*?\n\}  \/\/ namespace detail/g, '');
  const entries = [];
  for (const match of publicSource.matchAll(/^(using|struct|class)\s+([A-Za-z][A-Za-z0-9_]*)/gm)) {
    const rawKind = match[1];
    const name = match[2];
    if (name === 'ModelSelection' && publicSource.slice(match.index, match.index + 40).includes(';')) continue;
    entries.push({
      name: `cityjson_lib::${name}`,
      kind: rawKind === 'using' ? 'type alias' : rawKind === 'class' ? 'class' : 'struct',
      aliases: [name],
      signature: firstLineAt(publicSource, match.index),
    });
  }
  for (const classMatch of publicSource.matchAll(/^class\s+([A-Za-z][A-Za-z0-9_]*)[^{]*\{/gm)) {
    const className = classMatch[1];
    const bodyStart = publicSource.indexOf('{', classMatch.index) + 1;
    const bodyEnd = matchingBrace(publicSource, bodyStart - 1);
    const body = publicSource.slice(bodyStart, bodyEnd).split(' private:')[0];
    for (const method of body.matchAll(/(?:\[\[nodiscard\]\]\s+)?(?:static\s+)?(?:[A-Za-z_:<>,&*\s]+)\s+([A-Za-z][A-Za-z0-9_]*)\s*\(/gm)) {
      const methodName = method[1];
      if (['if', 'for', 'while', 'return', className].includes(methodName)) continue;
      entries.push({
        name: `cityjson_lib::${className}::${methodName}`,
        kind: 'method',
        aliases: [`${className}::${methodName}`, methodName],
        signature: compactCppSignature(publicSource, bodyStart + method.index),
      });
    }
  }
  return entries;
}

function wasmApi(source) {
  const entries = [];
  for (const match of source.matchAll(/^pub\s+struct\s+([A-Za-z][A-Za-z0-9_]*)/gm)) {
    entries.push({ name: match[1], kind: 'struct', signature: firstLineAt(source, match.index) });
  }
  for (const match of source.matchAll(/^pub\s+fn\s+([A-Za-z][A-Za-z0-9_]*)/gm)) {
    entries.push({ name: match[1], kind: 'function', signature: compactSignature(source, match.index) });
  }
  return entries;
}

function rustDocsRsUrl(target, name, kind) {
  const cratePrefix = target.package.replace('-', '_');
  const pathName = name.replace(`${cratePrefix}::`, '');
  const parts = pathName.split('::');
  const last = parts.at(-1);
  if (kind === 'method' && parts.length >= 2) {
    const typeName = parts.at(-2);
    return `${target.docsRsBase}struct.${typeName}.html#method.${last}`;
  }
  if (kind === 'struct') return `${target.docsRsBase}struct.${last}.html`;
  if (kind === 'enum') return `${target.docsRsBase}enum.${last}.html`;
  if (kind === 'type alias') return `${target.docsRsBase}type.${last}.html`;
  if (kind === 'constant') return `${target.docsRsBase}constant.${last}.html`;
  if (kind === 'function') return `${target.docsRsBase}fn.${last}.html`;
  return target.docsRsBase;
}

function firstLineAt(source, index) {
  return source.slice(index, source.indexOf('\n', index)).trim();
}

function compactSignature(source, index) {
  const end = source.indexOf('{', index);
  return source.slice(index, end === -1 ? source.indexOf('\n', index) : end).replace(/\s+/g, ' ').trim();
}

function compactPythonSignature(source, index) {
  let depth = 0;
  for (let cursor = index; cursor < source.length; cursor += 1) {
    const char = source[cursor];
    if (char === '(') depth += 1;
    if (char === ')') depth -= 1;
    if (char === ':' && depth === 0) {
      return source.slice(index, cursor + 1).replace(/\s+/g, ' ').trim();
    }
  }
  return firstLineAt(source, index);
}

function compactCppSignature(source, index) {
  const endCandidates = ['{', ';'].map((token) => source.indexOf(token, index)).filter((value) => value !== -1);
  const end = Math.min(...endCandidates);
  return source.slice(index, end).replace(/\s+/g, ' ').trim();
}

function nextTopLevel(source, start) {
  const match = /\n(?=class |def |[A-Za-z_][A-Za-z0-9_]*\s*=|__all__)/g;
  match.lastIndex = start;
  const next = match.exec(source);
  return next ? next.index : source.length;
}

function matchingBrace(source, openIndex) {
  let depth = 0;
  for (let index = openIndex; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return source.length;
}


function assignUniqueAnchors(entries) {
  const seen = new Map();
  return entries.map((entry) => {
    const count = seen.get(entry.anchor) ?? 0;
    seen.set(entry.anchor, count + 1);
    return count === 0 ? entry : { ...entry, anchor: `${entry.anchor}-${count + 1}` };
  });
}

function groupBy(values, keyFn) {
  const groups = new Map();
  for (const value of values) {
    const key = keyFn(value);
    groups.set(key, [...(groups.get(key) ?? []), value]);
  }
  return groups;
}

function uniqueBy(values, keyFn) {
  const seen = new Set();
  const unique = [];
  for (const value of values) {
    const key = keyFn(value);
    if (seen.has(key)) continue;
    seen.add(key);
    unique.push(value);
  }
  return unique;
}

function slugify(value) {
  return value
    .replace(/::/g, '-')
    .replace(/\./g, '-')
    .replace(/[^A-Za-z0-9_-]+/g, '-')
    .replace(/^-|-$/g, '')
    .toLowerCase();
}

function kindLabel(kind) {
  const labels = {
    class: 'Classes',
    struct: 'Structs',
    enum: 'Enums',
    'type alias': 'Type aliases',
    constant: 'Constants',
    function: 'Functions',
    method: 'Methods',
  };
  return labels[kind] ?? `${kind[0].toUpperCase()}${kind.slice(1)}s`;
}

function fenceLanguage(language) {
  return language === 'cpp' ? 'cpp' : language === 'wasm' ? 'rust' : language;
}

function escapeMd(value) {
  return value.replaceAll('`', '\\`');
}
