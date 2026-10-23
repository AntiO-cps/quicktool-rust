const fs = require('node:fs'), path = require('node:path'), vm = require('node:vm');
const root = path.resolve(__dirname, '..');
const source = fs.readFileSync(path.join(root, '../src/renderer.js'), 'utf8');
const Core = require('../../src/core-utils.js');
function span(start, end) {
  const a = source.indexOf(start), b = source.indexOf(end, a + start.length);
  if (a < 0 || b < 0) throw Error(`Original source boundary changed: ${start}`);
  return source.slice(a, b);
}
const dummy = () => [];
const context = vm.createContext({
  Core, usage: {}, Date: { now: () => 1792800000000 },
  colorToolResults: dummy, caseResults: dummy, base64ToolResults: dummy,
  unicodeResults: dummy, countryResults: dummy, textResults: dummy, helpResults: dummy,
});
vm.runInContext([
  span('const norm=', 'function highlightFuzzy('),
  span('function usageBoost(', 'function markUsed('),
  span('const tools=[', 'const windowsSettings='),
  span('const systemCommands=[', 'function getTool('),
  span('function aliasScore(', 'function pushUnique('),
  span('function parseToolArg(', 'function buildCandidates('),
  'globalThis.allTools=[...tools,...systemCommands.map(x=>x.tool)];',
].join('\n'), context);
const tokens = new Set(['', 'a', 'ch', 'co', 's', '空', '\u0085', '\uFEFF', '---']);
for (const tool of context.allTools) for (const alias of tool.aliases) {
  tokens.add(alias); tokens.add(alias.toUpperCase()); tokens.add(alias.slice(0, 2));
  tokens.add(alias.slice(0, 3));
}
const matching = [...tokens].map(input => ({
  input,
  matches: context.commandMatches(input).map(x => [x.tool.id, x.alias, x.score])
}));
const parsing = ['/country\tch', '/country   ch', '/base64 dec aGVsbG8=', '/base64\t\tdecode  aGVsbG8=',
  '/country\nch', '/country\u00A0ch', '/country\uFEFFch', '/country\u0085ch', '/ country', '/', '/country',
  '/base64 decode\r\n aGVsbG8=', '/case  a  b ', '/country\u3000ch'].map(input => {
  const parsed = context.commandParse(input);
  return { input, token: parsed.token, arg: parsed.arg, exact: parsed.exact?.id ?? null };
});
const submodes = [];
for (const tool of context.allTools) {
  const inputs = new Set(['', '  ', 'ch', '  ch', 'dec aGVsbG8=', '  dec\t\t aGVsbG8=', 'deco aGVsbG8=',
    'en aGVsbG8=', 'd aGVsbG8=', '/dec aGVsbG8=', 'admin echo hi', 'ad echo hi', '\u0085decode x',
    '\uFEFFdecode\uFEFF\uFEFFaGVsbG8=', 'decode a  b ', 'decode\n\n  aGVsbG8=']);
  for (const sub of tool.subcommands || []) for (const alias of sub.aliases) {
    inputs.add(alias); inputs.add(alias.toUpperCase() + '\t value ');
    inputs.add(alias.slice(0, 2) + '   value');
  }
  for (const input of inputs) {
    const parsed = context.parseToolArg(tool, input);
    submodes.push({ tool: tool.id, input, submode: parsed.submode?.id ?? null, value: parsed.input });
  }
}
context.usage = { 'system:control': { count: 512, last: 1792800000000 } };
const usage = {
  input: '', boosts: { 'system:control': context.usageBoost('system:control') },
  matches: context.commandMatches('').slice(0, 16).map(x => [x.tool.id, x.alias, x.score]),
};
if (usage.matches[0][0] !== 'system:control') throw Error('Usage fixture must promote a result beyond initial 16');
const fixture = { matching, parsing, submodes, usage };
fs.writeFileSync(path.join(root, 'tests/command-matching-oracle.json'), JSON.stringify(fixture));
console.log(JSON.stringify({ matching: matching.length, parsing: parsing.length, submodes: submodes.length }));
