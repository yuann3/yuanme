import type { MountContext } from "yuanme:mount";

/** A copy button on each code block of a Page that calls `code_copy`. */
export function mount(ctx: MountContext) {
  const status = document.querySelector<HTMLElement>("[data-code-copy]");
  if (!status) return;
  const label = status.dataset.codeCopy ?? "Copy";
  const copied = status.dataset.copied ?? "Copied";
  for (const pre of document.querySelectorAll<HTMLElement>("pre.y-code")) {
    const block = document.createElement("div");
    block.className = "code-block";
    pre.replaceWith(block);
    const button = document.createElement("button");
    button.type = "button";
    button.className = "code-copy";
    button.textContent = label;
    block.append(pre, button);
    let reset: ReturnType<typeof setTimeout> | undefined;
    button.addEventListener(
      "click",
      async () => {
        await navigator.clipboard.writeText(pre.querySelector("code")?.textContent ?? "");
        button.textContent = copied;
        status.textContent = copied;
        clearTimeout(reset);
        reset = setTimeout(() => {
          button.textContent = label;
          status.textContent = "";
        }, 2000);
      },
      { signal: ctx.signal },
    );
  }
}
