// Read the original implementation directly; never execute reference JS in production.
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const root = path.resolve(__dirname, '..');
const Core = require(path.join(root, '../src/core-utils.js'));
const dataset = require(path.join(root, '../src/data/countries.json'));
const renderer = fs.readFileSync(path.join(root, '../src/renderer.js'), 'utf8');
const countries = Core.countryIndex(dataset);
const context = { Core, countries };
const cardStart = renderer.indexOf('function countryCardItem(');
const cardEnd = renderer.indexOf('\nfunction appSearch(', cardStart);
const detectStart = renderer.indexOf('detect:q=>{if(!countries.length)');
const detectEnd = renderer.indexOf(',run:countryResults}', detectStart);
if ([cardStart, cardEnd, detectStart, detectEnd].some(x => x < 0)) throw Error('Country source boundary changed');
vm.runInNewContext(renderer.slice(cardStart, cardEnd) +
  `\nglobalThis.detect = ({${renderer.slice(detectStart, detectEnd)}}).detect;`, context);
const inputs = new Set(['ch', 'CH', 'Ch', '.ch', '+41', '瑞士', 'Swiss', 'Switzerland',
  '', ' ', '不存在', 'zzzzzz', '+99999', '.', '+', 'a', 'c', 'congo', 'korea', 'united',
  'republic', 'island', 'cote', 'Côte', '瑞', 'CHINA', '\uFEFFChina\uFEFF', 'China\u0085',
  'Chin\uFEFFa', 'Ｓｗｉｔｚｅｒｌａｎｄ', 'ＣＨ', 'Swi-tzer_land', '  ch  ']);
for (const country of dataset.countries) {
  for (const key of ['nameZh', 'officialNameZh', 'nameEn', 'officialNameEn', 'alpha2', 'alpha3', 'numeric', 'ioc', 'fifa', 'itu', 'callingCodes', 'tlds', 'aliases']) {
    for (const value of [country[key]].flat().filter(Boolean).map(String)) {
      inputs.add(value);
      inputs.add(value.toLowerCase());
      inputs.add(value.toUpperCase());
      if (['alpha2', 'alpha3'].includes(key)) {
        inputs.add(value[0].toUpperCase() + value.slice(1).toLowerCase());
        inputs.add(` \uFEFF${value.toLowerCase()}\uFEFF `);
      }
      if (key === 'callingCodes') inputs.add('+' + value.replace(/^\+/, ''));
      if (['nameEn', 'nameZh', 'aliases'].includes(key) && value.length > 3) {
        inputs.add(value.slice(0, 3));
        inputs.add(value.slice(1, -1));
        inputs.add(value.slice(0, -1) + '~');
      }
    }
  }
}
const cases = [...inputs].map(input => ({
  input,
  matches: Core.findCountries(countries, input, 8).map(({ country, score }) => [country.alpha2, score]),
  detection: context.detect(input)?.score ?? null,
  completions: Core.findCountries(countries, input, 4).map(({ country }) => [
    country.alpha2, context.countryCompletion(country, input)
  ]),
  cards: (() => { try { return context.countryResults(input).map(row => [
    (row.country || row.countryNavigate).alpha2, row.score
  ]); } catch { return []; } })()
}));
fs.writeFileSync(path.join(root, 'tests/countries-oracle.json'), JSON.stringify({
  source: 'src/core-utils.js countryIndex/findCountries and src/renderer.js countryResults/detect',
  countries: dataset.countries.length,
  cases
}));
console.log(`Captured ${cases.length} country queries across ${dataset.countries.length} countries`);
