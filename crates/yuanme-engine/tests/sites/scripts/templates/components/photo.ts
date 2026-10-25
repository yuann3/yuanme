import { greet } from "/scripts/shared.ts";

export function mount(ctx: { url: URL; signal: AbortSignal }): () => void {
  for (const photo of document.querySelectorAll(".photo")) {
    photo.addEventListener("click", () => greet(ctx.url.pathname), { signal: ctx.signal });
  }
  return () => greet("bye");
}
