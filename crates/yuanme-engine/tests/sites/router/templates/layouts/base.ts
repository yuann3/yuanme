// Every Page renders the layout, so every Page mounts this: a copy button
// on each code block, as the Reference site's copy button does.
const w = window as any;
w.__top = (w.__top ?? 0) + 1;
w.__life ??= [];

export function mount(ctx: { url: URL; signal: AbortSignal; data: { mounts?: number } }) {
  ctx.data.mounts = (ctx.data.mounts ?? 0) + 1;
  w.__life.push(`mount ${ctx.url.pathname} ${ctx.data.mounts}`);
  for (const pre of document.querySelectorAll("main pre")) {
    const button = document.createElement("button");
    button.className = "copy";
    button.textContent = "Copy";
    button.addEventListener("click", () => (w.__copied = pre.textContent), { signal: ctx.signal });
    pre.append(button);
  }
  ctx.signal.addEventListener("abort", () => w.__life.push(`abort ${ctx.url.pathname}`));
  return () => w.__life.push(`cleanup ${ctx.url.pathname}`);
}
