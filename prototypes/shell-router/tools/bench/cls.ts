import { chromium } from 'playwright';
const b = await chromium.launch();
for (const port of [4321, 4322]) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
  await ctx.addInitScript({ path: import.meta.dir + '/probe.js' });
  const page = await ctx.newPage();
  await page.goto('http://localhost:' + port + '/'); await page.waitForTimeout(1500);
  const mark = async (label: string) => { await page.waitForTimeout(1500); const s = await page.evaluate(() => { const P = (window as any).__probe; const r = P.shifts.splice(0); return r.map((x: any) => `${x[1].toFixed(3)}${x[2] ? '(input)' : ''}[${x[3]}]`).join(' '); }); console.log(port, label, s); };
  await mark('cold');
  await page.click('[data-nav-link][data-section=writing]'); await mark('A');
  await page.click('[data-nav-link][data-section=projects]'); await mark('B');
  await page.click('[data-panel-content] a.close-desktop'); await mark('C');
  await page.evaluate(() => history.back()); await mark('back');
  await ctx.close();
}
await b.close();
