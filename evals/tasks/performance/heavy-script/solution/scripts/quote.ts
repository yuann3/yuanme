// The quotes load after the Page, in a chunk of their own, which the JS
// budget doesn't count.
const { QUOTES } = await import('./quotes.ts');
const day = Math.floor(Date.now() / 86_400_000);
const el = document.getElementById('quote');
if (el) el.textContent = QUOTES[day % QUOTES.length];

export {};
