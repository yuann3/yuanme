# A 1600×340 photo-like PNG, stored without compression: 1.6 MB.
node --input-type=module -e '
import { writeFileSync } from "node:fs";
import { deflateSync, crc32 } from "node:zlib";
const w = 1600, h = 340;
const raw = Buffer.alloc((w * 3 + 1) * h);
let seed = 7;
const noise = () => ((seed = (seed * 1103515245 + 12345) >>> 0) >>> 24) % 9;
for (let y = 0; y < h; y++) {
  const row = y * (w * 3 + 1);
  for (let x = 0; x < w; x++) {
    const i = row + 1 + x * 3;
    raw[i] = 40 + (x * 160) / w + noise();
    raw[i + 1] = 90 + (y * 120) / h + noise();
    raw[i + 2] = 200 - (x * 80) / w + noise();
  }
}
const chunk = (type, data) => {
  const out = Buffer.alloc(12 + data.length);
  out.writeUInt32BE(data.length, 0);
  out.write(type, 4, "ascii");
  data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
};
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
writeFileSync("static/hero.png", Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw, { level: 0 })), chunk("IEND", Buffer.alloc(0)),
]));
'
perl -pi -e 's|^<h1>\{\{ site.title \| default\("Home"\) \}\}</h1>$|<img class="hero" src="/hero.png" alt="Hills at sunrise" width="1600" height="340">\n<h1>{{ site.title \| default("Home") }}</h1>|' templates/home.html
grep -q 'class="hero"' templates/home.html
test "$(wc -c < static/hero.png)" -gt 1600000
