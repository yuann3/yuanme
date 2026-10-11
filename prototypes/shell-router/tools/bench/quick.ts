import { chromium } from 'playwright';
const port = +(process.argv[2] ?? 4322);
const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
await ctx.addInitScript({ path: import.meta.dir + '/probe.js' });
const page = await ctx.newPage();

page.on('pageerror', (e) => console.log('pageerror:', e.message));
const base = `http://localhost:${port}`;
const waitDone = async () => {
  for (let i = 0; i < 200; i++) {
    try { const d = await page.evaluate(() => (window as any).__probe?.done); if (d) return d; } catch {}
    await page.waitForTimeout(50);
  }
  return 'TIMEOUT';
};
const clr = () => page.evaluate(() => { (window as any).__probe.done = null; });
const snap = () => page.evaluate(() => {
  const P = (window as any).__probe;
  return { url: location.pathname, title: document.title, state: document.documentElement.dataset.state, section: document.documentElement.dataset.section,
    cur: [...document.querySelectorAll('[data-nav-link]')].map((a) => a.getAttribute('aria-current')).join(','),
    ev: P.events.map((e: any) => e[0]).join('>'), listeners: P.listeners.join(','), glcalls: P.gl.calls, ctxTab: sessionStorage.getItem('__ctx'),
    canvasId: (document.querySelector('[data-film-canvas]') as any)?.__probeId, lost: P.gl.ctx?.isContextLost(), contents: document.querySelectorAll('[data-panel-content]').length,
    copy: document.querySelectorAll('[data-code-block]').length, pres: document.querySelectorAll('[data-panel-content] pre').length, entries: (window as any).navigation?.entries().length ?? history.length,
    announcers: document.querySelectorAll('.astro-route-announcer').length, done: P.done };
});
await page.goto(base + '/'); console.log('cold', JSON.stringify(await waitDone()));
await page.waitForTimeout(600);
await clr(); await page.click('[data-nav-link][data-section=writing]'); await waitDone(); console.log('A', JSON.stringify(await snap()));
await page.waitForTimeout(600);
await clr(); await page.click('[data-panel-content] a[href*="/blog/extending-c-stdlib"]'); await waitDone(); await page.waitForTimeout(300); console.log('post', JSON.stringify(await snap()));
await page.waitForTimeout(600);
await page.evaluate(() => { (window as any).__probe.done = null; sessionStorage.setItem('__act', String(performance.timeOrigin + performance.now())); history.back(); });
await waitDone(); console.log('back', JSON.stringify(await snap()));
await page.waitForTimeout(600);
await clr(); await page.click('[data-nav-link][data-section=projects]'); await waitDone(); console.log('B', JSON.stringify(await snap()));
await page.waitForTimeout(600);
await clr(); await page.click('[data-panel-content] a.close-desktop'); await waitDone(); console.log('C', JSON.stringify(await snap()));
await page.waitForTimeout(600);
await clr(); await page.click('[data-nav-link][data-section=about]'); await waitDone(); await page.waitForTimeout(1500); console.log('about', JSON.stringify(await snap()), await page.evaluate(() => document.querySelectorAll('[data-pile][data-init]').length));
await clr(); await page.click('a[href="/rss.xml"]'); await page.waitForTimeout(1000); console.log('rss', page.url());
await b.close();
