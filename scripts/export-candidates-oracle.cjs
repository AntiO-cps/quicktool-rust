// Offline oracle: execute the original renderer candidate pipeline without a DOM.
// Only host state is synthetic. No QuickTool source or user settings are changed.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '..');
const rendererPath = path.resolve(root, '../src/renderer.js');
const source = fs.readFileSync(rendererPath, 'utf8');
const Core = require('../../src/core-utils.js');
const DateParser = require('../../src/date-parser.js');
const HelpCatalog = require('../../src/help-catalog.js');
function span(start, end) {
  const a = source.indexOf(start), b = source.indexOf(end, a + start.length);
  assert(a >= 0 && b > a, `Original renderer anchors changed: ${start}`);
  return source.slice(a, b);
}
const originalCode = [
  span('const norm=', 'function highlightFuzzy('),
  span('function usageBoost(', 'function markUsed('),
  span('function titleCase(', 'function setImageAttachment('),
  span('function base64Decode(', 'const tools=['),
  span('const tools=[', 'function setResultTab('),
  span('function fileSearchItems(', 'function requestFileSearch('),
  span('function runTool(', 'function renderTags('),
].join('\n');
const fixedNow = Date.parse('2026-10-24T00:00:00Z');
class FixtureDate extends Date {
  constructor(...args) { super(...(args.length ? args : [fixedNow])); }
  static now() { return fixedNow; }
}
const context = vm.createContext({
  Core, DateParser, HelpCatalog, Date: FixtureDate, Intl,
  navigator: { language: 'en-US' }, atob, btoa,
  crypto: { randomUUID: () => '00000000-0000-4000-8000-000000000000', getRandomValues: a => a.fill(1) },
  settings: { dateOrder: 'YMD', timeCustomFormats: '', fileSearchGlobal: true },
  countries: Core.countryIndex(require('../assets/data/countries.json')),
  cssColors: require('../assets/data/css-colors.json'),
  unicodeMeta: require('../assets/data/unicode-meta.json'), unicodeSearchIndex: null,
  mode: { toolId: null, args: [] }, usage: {}, activeResultTab: 'all',
  appIndex: [], knownFolders: [], clipboardSnapshot: [], inputAttachment: null,
  fileSearchState: { query: '', pending: false, items: [], provider: 'fixture' },
  fileIndexState: { building: false, count: 0 },
  registrySearchState: { query: '', root: 'all', pending: false, items: [] },
  pinBtn: { classList: { contains: () => false } },
});
vm.runInContext(originalCode + '\nglobalThis.oracleTools=tools; globalThis.oracleSystems=systemCommands;', context, { filename: rendererPath });
function compact(row) {
  const result = { tool: row.tool?.id || '', title: row.title, value: row.value, score: row.score, category: context.candidateCategory(row) };
  for (const key of ['subValue', 'commandSuggestion', 'matchedAlias', 'modeAction', 'modeStatus', 'countryCard', 'launcherAction', 'shellAction', 'systemAction', 'openAction', 'pathAction', 'fileAction', 'registryAction', 'refreshClipboard', 'literal', 'helpEntry']) {
    if (row[key] !== undefined) result[key] = row[key];
  }
  if (row.country) result.country = row.country.alpha2;
  if (row.countryNavigate) result.countryNavigate = row.countryNavigate.alpha2;
  if (row.appAction) result.appAction = row.appAction.name;
  return result;
}
function evaluate(spec) {
  context.mode = { toolId: spec.mode || null, args: spec.submode ? [{ id: spec.submode, label: spec.submode }] : [] };
  context.activeResultTab = spec.tab || 'all';
  context.usage = spec.usage || {};
  context.settings = { dateOrder: 'YMD', timeCustomFormats: '', fileSearchGlobal: true, ...spec.settings };
  context.appIndex = spec.apps || [];
  context.knownFolders = spec.folders || [];
  context.clipboardSnapshot = spec.clipboard || [];
  context.fileSearchState = { query: '', pending: false, items: [], provider: 'fixture', ...spec.files };
  context.registrySearchState = { query: '', root: 'all', pending: false, items: [], ...spec.registry };
  context.inputAttachment = spec.attachment || null;
  const raw = context.buildCandidates(spec.input);
  let visible = context.filterCandidatesForUX(raw, spec.input);
  // Original render() performs this grouping AFTER filtering and truncation.
  if (context.activeResultTab === 'all' && spec.input.trim() && visible.length > 1 && !visible.every(c => c.helpEntry)) {
    const [top, ...rest] = visible;
    visible = [top, ...rest.filter(c => context.candidateCategory(c) === 'run'), ...rest.filter(c => context.candidateCategory(c) === 'tools')];
  }
  return { ...spec, raw: raw.map(compact), visible: visible.map(compact) };
}
function createOracle() {
  const specs = [];
  const add = (input, extra = {}) => specs.push({ input, ...extra });
  for (const input of ['', ' ', '\u0085', 'ch\u0085', '\u0085ch', 'ch\uFEFF', '/country\uFEFFch', 'ch', 'CH', 'chn', 'CN', '.ch', '+41', '756', '瑞士', 'Switzerland', '中国', 'China', 'Congo', 'Korea', 'United', 'case', 'country', 'settings', 'files', 'chro', 'hello', 'hello world', 'a\nb', '1+2', '0xFF', '10 km to mi', '1 GB', '#7c3aed', 'red', '260120', '20260120', '2026-01-20', '1700000000', 'true', 'null', '{"a":1}', 'aGVsbG8=', '%E4%B8%AD', 'U+1F600', 'ＡＢＣ', 'https://example.test/a', '&amp;']) add(input);
  for (const tool of context.oracleTools) add('/' + tool.primaryAlias);
  for (const system of context.oracleSystems) add('/' + system.aliases[0] + (system.needsArg ? ' example.test' : ''));
  for (const input of ['/country ch', '/country\tch', '/country  ch', '/country\nch', '/base64 decode aGVsbG8=', '/base64 dec aGVsbG8=', '/base64   decode aGVsbG8=', '/base64\tdecode aGVsbG8=', '/cmd adm echo hi', '/cmd admin', '/ping', '/help ch', '/winsettings display', '/notacommand']) add(input);
  for (const mode of ['country', 'base64', 'json', 'time', 'color', 'cmd', 'help', 'files', 'registry', 'run', 'pin', 'clipboard']) {
    for (const input of ['', mode === 'country' ? 'ch' : mode === 'base64' ? 'aGVsbG8=' : mode === 'time' ? '2026-01-20' : mode === 'json' ? '{"a":1}' : mode === 'color' ? '#123456' : 'hello']) add(input, { mode });
  }
  for (const input of ['/dec', '/deco', '/DECODE', '\\/decode', 'aGVsbG8=']) add(input, { mode: 'base64' });
  for (const tab of ['all', 'run', 'tools']) for (const input of ['ch', 'CH', '+41', 'hello', 'case', '0xFF', '/country ch', '/help']) add(input, { tab });
  const apps = [{ name: 'Chrome', source: 'fixture application', target: 'C:\\fixture\\chrome.exe' }, { name: 'Character Map', source: 'fixture application', target: 'C:\\fixture\\charmap.exe' }, { name: '123', source: 'fixture application', target: 'C:\\fixture\\123.exe' }];
  for (const input of ['ch', 'chro', '123']) for (const tab of ['all', 'run', 'tools']) add(input, { apps, tab });
  const manyApps = Array.from({ length: 20 }, (_, i) => ({ name: `Search Application ${i}`, source: 'fixture application', target: `C:\\fixture\\search${i}.exe` }));
  add('Search', { mode: 'run', apps: manyApps });
  add('unmatched', { mode: 'run', apps: manyApps });
  add('', { mode: 'run', apps: manyApps });
  const items = Array.from({ length: 8 }, (_, i) => ({ name: `ch${i}.txt`, path: `C:\\fixture\\ch${i}.txt`, provider: 'fixture' }));
  for (const query of ['ch', 'older query']) for (const tab of ['all', 'run', 'tools']) add('ch', { tab, apps, files: { query, items } });
  add('ch', { settings: { fileSearchGlobal: false }, files: { query: 'ch', items } });
  add('ch', { mode: 'files', files: { query: 'ch', items } });
  add('ch', { mode: 'files', files: { query: 'older query', items } });
  add('report', { files: { query: 'report', items: items.map((v, i) => ({ ...v, name: `report${i}.txt` })) } });
  add('HKCU hello', { mode: 'registry', registry: { query: 'hello', root: 'HKCU', items: [{ kind: 'value', path: 'HKCU\\fixture', name: 'hello', data: 'sample', valueType: 'REG_SZ', provider: 'fixture' }] } });
  for (const input of ['', 'ch', 'case', '/ch', 'cn']) add(input, { usage: { country: { count: 512, last: fixedNow }, case: { count: 64, last: fixedNow - 86400000 }, char: { count: 128, last: fixedNow } } });
  for (const input of ['a', '/a']) for (const boosted of [false, true]) add(input, { usage: boosted ? { country: { count: 512, last: fixedNow } } : {} });
  add('', { mode: 'clipboard', clipboard: [
    { type: 'text', format: 'CF_UNICODETEXT', value: 'clipboard text', preview: 'unused preview', byteLength: 28 },
    { type: 'text', format: 'FixturePreview', value: '', preview: 'preview fallback', byteLength: 12 },
  ] });
  add('', { mode: 'clipboard', clipboard: [
    { type: 'binary', format: 'FixtureBytes', value: '', preview: '', base64: 'AAEC', bytes: [0, 1, 2], byteLength: 3 },
    { type: 'binary', format: 'FixtureLength', value: '', preview: '', bytes: [], byteLength: 11 },
  ] });
  const pixelPng = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jXioAAAAASUVORK5CYII=';
  add('', { mode: 'clipboard', clipboard: [
    { type: 'image', format: 'PNG', value: '', preview: '', base64: pixelPng, bytes: [...Buffer.from(pixelPng, 'base64')], byteLength: Buffer.from(pixelPng, 'base64').length, width: 1, height: 1, dataUrl: `data:image/png;base64,${pixelPng}` },
    { type: 'image', format: 'FixtureImagePreview', value: '', preview: 'image preview', base64: pixelPng, bytes: [...Buffer.from(pixelPng, 'base64')], byteLength: Buffer.from(pixelPng, 'base64').length, width: 1, height: 1 },
  ] });
  const cases = specs.map(evaluate);
  assert.equal(cases.find(c => c.input === 'ch' && !c.tab && !c.apps && !c.usage).visible[0].country, 'CH');
  assert.equal(cases.find(c => c.input === 'ch' && !c.tab && !c.apps && !c.usage).visible[0].score, 1.15);
  assert.equal(cases.find(c => c.input === '/country\tch').visible[0].country, 'CH');
  assert(cases.find(c => c.input === 'ch' && c.files?.query === 'older query').raw.every(r => !r.fileAction));
  return {
    provenance: {
      originalRendererSha256: crypto.createHash('sha256').update(source).digest('hex'),
      originalCoreSha256: crypto.createHash('sha256').update(fs.readFileSync(path.resolve(root, '../src/core-utils.js'))).digest('hex'),
      runtime: process.versions.node, timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
      fixedNow: new Date(fixedNow).toISOString(),
      fixedNowMs: fixedNow,
      realFunctions: ['buildCandidates', 'buildModeCandidates', 'filterCandidatesForUX', 'commandParse', 'commandMatches', 'pushUnique', 'parseToolArg', 'subcommandMatch', 'countryResults', 'appSearch', 'fileSearchItems', 'registrySearchItems', 'runTool', 'original tools/system definitions', 'Core', 'DateParser', 'HelpCatalog'],
      fixtureHostState: ['applications/folders/file/registry/clipboard state', 'pinned=false', 'navigator.language=en-US', 'fixed Date.now', 'deterministic random generator'],
      boundaries: 'No native OS search, clipboard, IME, screen rendering or animation. Pure candidate generation/filtering/grouping is original code. App/file/registry fixtures are supplied as input state, not fake search implementations.',
    },
    cases,
  };
}
if (require.main === module) {
  const oracle = createOracle();
  const destination = path.resolve(process.argv[2] || path.join(root, 'tests/candidates-oracle.json'));
  fs.writeFileSync(destination, JSON.stringify(oracle, null, 2) + '\n');
  console.log(JSON.stringify({ cases: oracle.cases.length, output: destination, bytes: fs.statSync(destination).size }));
}
module.exports = { createOracle, evaluate };
