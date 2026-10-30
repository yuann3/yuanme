import { QUOTES } from './quotes.ts';

const day = Math.floor(Date.now() / 86_400_000);
const el = document.getElementById('quote');
if (el) el.textContent = QUOTES[day % QUOTES.length];
