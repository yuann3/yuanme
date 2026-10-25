// Only some Pages render the counter, so they get a Page Script of their own.
export function mount(ctx: { data: { n?: number } }) {
  ctx.data.n = (ctx.data.n ?? 0) + 1;
  document.querySelector("[data-counter]")!.textContent = `mounted ${ctx.data.n}`;
}
