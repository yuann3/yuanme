## 1. Our render: provenance (every page)

29 pages, 68 distinct scoped selectors, 392 selector-element matches, 375 stamped elements.

PASS  every element carrying data-y-<id> was written in that id's template (0 wrong)
PASS  no element of rendered content (data-md) carries a scope attribute (0)
PASS  (a) scoped-subject selectors match only elements of their own template (0 foreign matches)
PASS  (b) scoped-subject selectors match no rendered-content / slot element (0)
      the same selectors without the attribute would also have matched 942 elements (917 of them inside rendered content): these are the leaks scoping prevents.
      :global() subjects (6 selectors) matched 69 elements; these are meant to reach content:
        components/photo_pile.html || .photo[S] picture  -> 8 matches, 0 in content, 21 more without the ancestor attribute
        components/photo_pile.html || .photo[S] img  -> 16 matches, 0 in content, 44 more without the ancestor attribute
        components/photo_pile.html |@media (prefers-color-scheme: dark)| .photo[S] img  -> 8 matches, 0 in content, 22 more without the ancestor attribute
        layouts/post.html || [S]article em  -> 16 matches, 16 in content, 27 more without the ancestor attribute
        layouts/post.html || [S]article i  -> 8 matches, 8 in content, 13 more without the ancestor attribute
        projects/entry.html || .project-body[S]>:first-of-typeh1  -> 13 matches, 13 in content, 22 more without the ancestor attribute

selectors with no match on any page (26); all need runtime state set by PhotoPile's script:
  components/pagination.html || .page-link:hover[S]
  components/photo_pile.html |@container about-body (width >= 800px)| .pile[S][data-drag-on] .photo[S]
  components/photo_pile.html || .photo[S][data-st="wait"]
  components/photo_pile.html || .photo[S][data-st="drop"]
  components/photo_pile.html |@media (hover: hover) and (pointer: fine)| .photo:hover[S]
  components/photo_pile.html |@media (hover: hover) and (pointer: fine)| .photo[S][data-press]
  components/photo_pile.html |@media (hover: hover) and (pointer: fine)| .photo[S][data-drag]
  components/photo_pile.html |@media (hover: hover) and (pointer: fine) @container about-body (width >= 800px)| .photo:hover[S]
  components/photo_pile.html |@media (hover: hover) and (pointer: fine) @container about-body (width >= 800px)| .photo[S][data-press]
  components/photo_pile.html |@media (hover: hover) and (pointer: fine) @container about-body (width >= 800px)| .photo[S][data-drag]
  components/photo_pile.html |@media (prefers-reduced-motion: reduce)| .photo[S][data-st="drop"]
  components/photo_pile.html |@media (prefers-reduced-motion: reduce)| .photo:hover[S]
  components/photo_pile.html |@media (prefers-reduced-motion: reduce)| .photo[S][data-press]
  components/photo_pile.html |@media (prefers-reduced-motion: reduce)| .photo[S][data-drag]
  components/photo_pile.html |@media (prefers-reduced-motion: reduce) @container about-body (width >= 800px)| .photo:hover[S]
  components/photo_pile.html |@media (prefers-reduced-motion: reduce) @container about-body (width >= 800px)| .photo[S][data-press]
  components/photo_pile.html |@media (prefers-reduced-motion: reduce) @container about-body (width >= 800px)| .photo[S][data-drag]
  components/photo_pile.html |@media (prefers-color-scheme: dark) @media (hover: hover) and (pointer: fine)| .photo:hover[S]
  components/photo_pile.html |@media (prefers-color-scheme: dark) @media (hover: hover) and (pointer: fine)| .photo[S][data-press]
  components/photo_pile.html |@media (prefers-color-scheme: dark) @media (hover: hover) and (pointer: fine)| .photo[S][data-drag]
  layouts/post.html || .meta-link:hover[S]
  projects/entry.html |@media (hover: hover)| .meta-link:hover[S]
  projects/entry.html |@media (hover: hover)| .meta-link:hover[S] .meta-arrow[S]
  projects/entry.html |@media (prefers-reduced-motion: reduce)| .meta-link:hover[S] .meta-arrow[S]
  projects/index.html |@media (hover: hover)| .project:hover[S] .project-year[S]
  projects/index.html |@media (hover: hover)| .project:focus-visible[S] .project-year[S]
PASS  every unmatched selector is state-dependent

## 2. Against the Reference's Astro build

### /blog/
  stamped elements, components/list_row.html: Astro 14, ours 18; kinds only in Astro: ["time.font-mono.leading-[18px].md:text-[13px].md:w-[140px].shrink-0.text-[12px].text-muted x2"]; only in ours: []
  stamped elements, components/pagination.html: Astro 0, ours 2; kinds only in Astro: []; only in ours: ["nav.flex.label-mono.pt-6.text-ink-2 x1","a.ms-auto.page-link x1"]
PASS  /blog/: 2/2 scoped-subject rules of this page match the same kinds of element as in Astro's output (0 with the same count; 2 more not exercised on one side)
### /about/
  stamped elements, components/photo_pile.html: Astro 19, ours 11; kinds only in Astro: ["picture x4","img x4"]; only in ours: []
PASS  /about/: 20/20 scoped-subject rules of this page match the same kinds of element as in Astro's output (20 with the same count; 19 more not exercised on one side)
### /projects/
  stamped elements, projects/index.html: Astro 118, ours 118; kinds only in Astro: []; only in ours: []
PASS  /projects/: 10/10 scoped-subject rules of this page match the same kinds of element as in Astro's output (10 with the same count; 2 more not exercised on one side)
### /projects/lrle/
  stamped elements, projects/entry.html: Astro 6, ours 9; kinds only in Astro: []; only in ours: ["img x1"]
PASS  /projects/lrle/: 2/2 scoped-subject rules of this page match the same kinds of element as in Astro's output (0 with the same count; 3 more not exercised on one side)
### /blog/extending-c-stdlib/
  stamped elements, layouts/post.html: Astro 8, ours 6; kinds only in Astro: ["time x1","img x1"]; only in ours: []
PASS  /blog/extending-c-stdlib/: 1/1 scoped-subject rules of this page match the same kinds of element as in Astro's output (1 with the same count; 1 more not exercised on one side)
PASS  selector sets equal after normalising the attribute name and simple-selector order: Astro 68, ours 68

## 3. Specificity dependencies (computed style, Chromium)

| Probe | Astro build | Ours (global CSS, then scoped) | Ours (scoped, then global CSS) | Control: same sidecars with the attribute removed, then global CSS |
|---|---|---|---|---|
| .project @375 (scoped align-items:flex-start vs .row center) | {align-items:flex-start,gap:12px,position:relative} | same | same | **{align-items:center,gap:12px,position:relative}** |
| .project-arrow @375 (extends .row-arrow) | {position:absolute,top:0px,right:0px,font-size:15px,opacity:0,margin-left:6px,display:flex} | same | same | same |
| .project @1280 (scoped media rule vs .row) | {align-items:center,gap:0px,padding-top:10px} | same | same | same |
| .list-row-arrow (extends .row-arrow) | {position:absolute,top:0px,font-size:18px,opacity:0,margin-left:6px,display:flex,color:rgb(85, 87, 93)} | same | same | same |
| .list-row-arrow, row hovered (.row-link:hover .row-arrow) | {opacity:1,transform:none} | same | same | same |
| .meta-link.text-ink-2 at rest | {color:rgb(85, 87, 93)} | same | same | same |
| .meta-link.text-ink-2 hovered (scoped :hover vs .text-ink-2) | {color:rgb(18, 19, 22),is --ink:true} | same | same | same |
PASS  (c) all probes resolve as in the Astro build with Astro's stylesheet order
PASS  (c) and still do with the scoped sheet loaded BEFORE the global one (the +0-1-0 carries them)

0 failures. Chromium 156.0.8078.4.
