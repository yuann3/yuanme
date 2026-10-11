| item | raw / gzip -9 / brotli q11 (B) |
|---|---|
| router.ts alone (Navigation API driver + swap core + scripts + prefetch + announcer + dev hook) | 3900 / 1808 / 1608 |
| router.ts without the dev hook | 3760 / 1770 / 1562 |
| router.ts without dev hook and prefetch | 3350 / 1585 / 1412 |
| router.ts + router-history.ts (History API driver) | 4863 / 2109 / 1879 |
| shell.ts bundle (film, palettes, reveal; router external) | 26640 / 10996 / 9798 |
| shell.ts bundle without glue (morph, syncNav, swapPanel, before-swap) | 23428 / 9915 / 8827 |
| shell.ts bundle without the dev branch of the glue | 26397 / 10913 / 9739 |
| glue alone (morph + syncNav + swapPanel + before-swap, motion-reveal external) | 3723 / 1698 / 1541 |
| astro-dist/_astro/router.CNTNUqWm.js | 11582 / 4086 / 3644 |
| astro-dist/_astro/Shell.astro_astro_type_script_index_0_lang.C3Ek_E9W.js | 26714 / 10982 / 9796 |
| astro-dist/_astro/PhotoPile.astro_astro_type_script_index_0_lang.Cmp7FTzI.js | 7219 / 3051 / 2751 |
| astro-dist/_astro/ClientRouter.astro_astro_type_script_index_0_lang.C8icoCZT.js | 3893 / 1686 / 1474 |
| proto-dist/_astro/PhotoPile.astro_astro_type_script_index_0_lang.Cmp7FTzI.js | 7219 / 3051 / 2751 |
| proto-dist/_astro/Shell.astro_astro_type_script_index_0_lang.DehljIte.js | 30733 / 12587 / 11178 |
| proto-hist-dist/_astro/Shell.astro_astro_type_script_index_0_lang.DJ1hmFPl.js | 31916 / 12958 / 11539 |
| proto-hist-dist/_astro/PhotoPile.astro_astro_type_script_index_0_lang.Cmp7FTzI.js | 7219 / 3051 / 2751 |

Per-page first-party JS (each file compressed separately, as served):
astro-dist index.html: JS requests 3; external 42189 / 16754 / 14914; inline 264 / 200 / 136; total 42453 / 16954 / 15050
astro-dist blog/index.html: JS requests 3; external 42189 / 16754 / 14914; inline 264 / 200 / 136; total 42453 / 16954 / 15050
astro-dist blog/extending-c-stdlib/index.html: JS requests 3; external 42189 / 16754 / 14914; inline 1120 / 703 / 548; total 43309 / 17457 / 15462
astro-dist about/index.html: JS requests 4; external 49408 / 19805 / 17665; inline 264 / 200 / 136; total 49672 / 20005 / 17801
proto-dist index.html: JS requests 1; external 30733 / 12587 / 11178; inline 264 / 200 / 136; total 30997 / 12787 / 11314
proto-dist blog/index.html: JS requests 1; external 30733 / 12587 / 11178; inline 264 / 200 / 136; total 30997 / 12787 / 11314
proto-dist blog/extending-c-stdlib/index.html: JS requests 1; external 30733 / 12587 / 11178; inline 1120 / 703 / 548; total 31853 / 13290 / 11726
proto-dist about/index.html: JS requests 2; external 37952 / 15638 / 13929; inline 264 / 200 / 136; total 38216 / 15838 / 14065
proto-hist-dist index.html: JS requests 1; external 31916 / 12958 / 11539; inline 264 / 200 / 136; total 32180 / 13158 / 11675
proto-hist-dist blog/index.html: JS requests 1; external 31916 / 12958 / 11539; inline 264 / 200 / 136; total 32180 / 13158 / 11675
proto-hist-dist blog/extending-c-stdlib/index.html: JS requests 1; external 31916 / 12958 / 11539; inline 1120 / 703 / 548; total 33036 / 13661 / 12087
proto-hist-dist about/index.html: JS requests 2; external 39135 / 16009 / 14290; inline 264 / 200 / 136; total 39399 / 16209 / 14426
