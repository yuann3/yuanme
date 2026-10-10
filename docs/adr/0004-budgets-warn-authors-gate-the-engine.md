# Budgets warn Authors and gate the Engine

Every performance Budget has one set of numbers, but a breach does different things depending on whose Site it is. On an Author's Site it is a warning: the Engine doesn't dictate how someone builds their site, and `--strict-budgets` (or `budgets.strict` in Site config) is the only way to make it an error. In the Engine's own CI, the same numbers are Gates on the Reference site, the Starters and the Scale Site, and a breach fails the PR. Without that, the regressions budgets exist to catch would pass silently, such as a 1.6 MB passthrough image added by an agent. The Agent eval runs in strict mode, because it scores whether the agent built something fast. Decided in [What are yuanme's performance budgets?](https://github.com/yuann3/yuanme/issues/15).

## Considered Options

- **Hard caps for everyone** (rejected): forces one way of building a Site onto every Author.
- **Warnings everywhere, Engine CI included** (rejected): nothing would catch regressions in the Engine's own output.

## Consequences

- A Site with no budget config still gets the Starter numbers as warnings, so an agent sees a breach in `--json` without anyone setting anything. `budgets = false` turns them off.
- Correctness limits are not Budgets and stay errors whatever the mode. A parse that runs out of fuel fails its file.
