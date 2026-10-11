import { chromium, firefox, webkit } from 'playwright';
for (const [name, bt] of [['chromium', chromium], ['firefox', firefox], ['webkit', webkit]] as const) {
  try {
    const b = await bt.launch({ headless: true, timeout: 30000 });
    const p = await b.newPage();
    await p.setContent('<canvas id=c></canvas>');
    const info = await p.evaluate(() => {
      const gl = (document.getElementById('c') as HTMLCanvasElement).getContext('webgl');
      const ext = gl && gl.getExtension('WEBGL_debug_renderer_info');
      return { ua: navigator.userAgent, nav: 'navigation' in window, renderer: gl && ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : String(!!gl) };
    });
    console.log(name, b.version(), JSON.stringify(info));
    await b.close();
  } catch (e) { console.log(name, 'FAILED', String(e).slice(0, 600)); }
}
