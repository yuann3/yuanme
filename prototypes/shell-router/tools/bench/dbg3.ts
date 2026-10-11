import { chromium } from 'playwright';
const b = await chromium.launch();
for (let i = 0; i < 3; i++) {
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
await ctx.addInitScript({ path: import.meta.dir + "/probe.js" });
const page = await ctx.newPage();
const list = () => page.evaluate(() => document.getAnimations().filter(a => a.playState === 'running' && isFinite(a.effect!.getComputedTiming().endTime as number)).map((a) => { const t = a.effect!.getComputedTiming(); const el = (a.effect as KeyframeEffect).target as Element; return `${(a as any).animationName || (a as any).transitionProperty || 'waapi'} ${el?.tagName}.${String(el?.className).slice(0,30)} end=${t.endTime} cur=${Math.round(a.currentTime as number)}`; }).join(' | '));
await page.goto('http://localhost:' + process.argv[2] + '/'); await page.waitForTimeout(1500);
await page.click('[data-nav-link][data-section=writing]'); await page.waitForTimeout(1200);
await page.click('[data-nav-link][data-section=projects]'); await page.waitForTimeout(1200);
await page.click('[data-panel-content] a.close-desktop'); await page.waitForTimeout(1200);
await page.evaluate(() => history.back()); await page.waitForTimeout(1200);
console.log('back+1.2s', await list());
await page.evaluate(() => history.forward()); for (let k = 0; k < 8; k++) { console.log("fwd+" + (k*400), await list(), JSON.stringify(await page.evaluate(() => (window as any).__probe.done))); await page.waitForTimeout(400); }
await ctx.close();
}
await b.close();
