# 1,500 quotes, as a list in the Script: about 20 kB of Brotli.
node --input-type=module -e '
import { writeFileSync } from "node:fs";
const words = "river lantern mountain patient quiet morning stone letter garden window bridge winter harbour candle orchard thread compass meadow ember silver".split(" ");
const verbs = "remembers keeps carries forgives teaches outlasts gathers mends answers follows".split(" ");
let seed = 11;
const pick = (list) => list[(seed = (seed * 1103515245 + 12345) >>> 0) % list.length];
const quotes = [];
for (let i = 1; i <= 1500; i++) {
  quotes.push(`Quote ${i}: the ${pick(words)} ${pick(verbs)} the ${pick(words)} that ${pick(words)}s ${pick(verbs).replace(/s$/, "")}.`);
}
writeFileSync("scripts/quotes.ts", `export const QUOTES: string[] = ${JSON.stringify(quotes, null, 1)};\n`);
'
perl -pi -e 's|^\{% if site.description %\}<p class="lede">\{\{ site.description \}\}</p>\{% endif %\}$|{% if site.description %}<p class="lede">{{ site.description }}</p>{% endif %}\n<p id="quote"></p>\n<script type="module" src="/scripts/quote.ts"></script>|' templates/home.html
grep -q 'src="/scripts/quote.ts"' templates/home.html
