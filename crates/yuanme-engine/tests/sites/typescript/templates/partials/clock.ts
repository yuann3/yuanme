import type { MountContext } from "yuanme:mount";

export function mount(ctx: MountContext<{ ticks?: number }>) {
  ctx.data.ticks = (ctx.data.ticks ?? 0) + 1;
  const timer = setInterval(() => {
    for (const el of document.querySelectorAll(".clock")) {
      el.textContent = ctx.url.pathname.toFixed();
    }
  }, 1000);
  ctx.signal.addEventListener("abort", () => clearInterval(timer));
  return () => clearInterval(timer);
}
