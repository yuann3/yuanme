import { chromium } from 'playwright';
const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
await ctx.addInitScript(() => {
  document.addEventListener('astro:after-swap', () => {
    const t0 = performance.now();
    requestAnimationFrame(() => requestAnimationFrame(() => {
      for (const a of document.getAnimations()) {
        const t = a.effect!.getComputedTiming(); if (!isFinite(t.endTime as number) || a.playState === 'finished') continue;
        const el = (a.effect as KeyframeEffect).target as Element;
        const name = `${(a as any).animationName || (a as any).transitionProperty || 'waapi'} ${el?.tagName}.${el?.className}`;
        a.finished.then(() => { const d = performance.now() - t0; if (d > 1000) console.log('slow', name, Math.round(d)); }, () => { const d = performance.now() - t0; if (d > 1000) console.log('slowrej', name, Math.round(d)); });
      }
    }));
  });
});
const page = await ctx.newPage();
page.on('console', (m) => console.log(m.text()));
await page.goto('http://localhost:' + process.argv[2] + '/'); await page.waitForTimeout(1500);
await page.click('[data-nav-link][data-section=writing]'); await page.waitForTimeout(1500);
await page.click('[data-panel-content] a.close-desktop'); await page.waitForTimeout(1500);
console.log('back'); await page.evaluate(() => history.back()); await page.waitForTimeout(1500);
console.log('fwd'); await page.evaluate(() => history.forward()); await page.waitForTimeout(5000);
await b.close();
