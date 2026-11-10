---
name: yuanme-verify
description: Check that a change to this yuanme Site works before calling it done. Runs yuanme check, builds the Site, reads the build report against the Site budgets, and looks at the Pages that changed. Use after editing content, Templates, styles, Scripts or yuanme.toml.
---

# Verify a change to the Site

Take the steps in order, and fix what one finds before going on to the next.

1. **Check.** Run `yuanme check --json`. Fix every `diagnostic` whose `level` is `error`, then each warning you caused. Apply a suggestion whose `applicability` is `machine-applicable` as it is; for any other, run `yuanme docs CODE` and fix what it names. Run it again until it exits 0.
2. **Build.** Run `yuanme build --json`. It reports what `check` does, a `budget` event for each Page over a Site budget, and writes `.yuanme/report.md`.
3. **Read the report.** `.yuanme/report.md` lists each Page's HTML, CSS, JS, fonts and requests against its Site budgets (`yuanme docs budgets`), and the modules in each Script bundle. For each Page over a budget:
   - Find what made it heavier: a stylesheet, Script, font or image the change added.
   - Make it smaller if you can: drop what the Page doesn't use, move a Script to the Pages that need it, or load it later.
   - Acknowledge the overage only when the weight is what the Page is for, such as an interactive demo or a Page of plots, and the Author agrees. Add `[budgets.overages."/its/url/"]` with the kB over, no more than the build reported. An Acknowledged overage stays listed in the report.
4. **Look at the Pages.** `yuanme routes --json` gives the URLs a changed file makes. If `yuanme dev status --json` says no dev server runs, start one with `yuanme dev --background --json` and take its URL from `server-ready`. Open each changed Page, or fetch its HTML and check that the change is there. `yuanme dev --write DIR` also writes each build to `DIR` for a look at the files.
5. **Report** the commands you ran and their exit codes, and each overage you acknowledged and why.
