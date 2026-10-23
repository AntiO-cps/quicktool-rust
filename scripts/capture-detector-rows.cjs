const fs = require('node:fs'), path = require('node:path'), vm = require('node:vm');
const root = path.resolve(__dirname, '..');
const source = fs.readFileSync(path.join(root, '../src/renderer.js'), 'utf8');
const Core = require('../../src/core-utils.js');
const cssColors = require('../assets/data/css-colors.json');
const context = vm.createContext({ Core, cssColors });
vm.runInContext(source.slice(source.indexOf('function colorToolResults('), source.indexOf('function colorParse(')), context);
const inputs = new Set(['#1', '#12', '#123', '#1234', '#112233', '#11223344', '#ff', '#fff',
  '#ab', '#abc', '#000', '#00', '#0f', '#abcde', 'no-such-color', '', '260120', '20260120',
  'rgba(1,2,3,.5)', 'rgb(255 0 0 / 50%)', 'transparent', ...Object.keys(cssColors)]);
const colors = [...inputs].map(input => {
  try { return { input, rows: context.colorToolResults(input).map(({title,value})=>({title,value})) }; }
  catch { return { input, error: true }; }
});
const { evaluate } = require('./export-candidates-oracle.cjs');
const detectors = ['260120', '20260120', '2026-01-20', '2026/1/5', '1700000000', '#123456', '#12', '#123'].map(input => ({
  input, rows: evaluate({input, settings:{timeCustomFormats:require('../assets/defaults.json').timeCustomFormats}}).raw.filter(row => ['time','color'].includes(row.tool) && !row.commandSuggestion)
    .map(row => ({ tool: row.tool, title: row.title, score: row.score }))
}));
fs.writeFileSync(path.join(root,'tests/detector-rows-oracle.json'),JSON.stringify({colors,detectors}));
console.log(JSON.stringify({colorQueries:colors.length,detectorQueries:detectors.length}));
