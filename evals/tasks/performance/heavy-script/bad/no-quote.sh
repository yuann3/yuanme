rm scripts/quote.ts scripts/quotes.ts
perl -ni -e 'print unless m{<script type="module" src="/scripts/quote.ts"></script>}' templates/home.html
