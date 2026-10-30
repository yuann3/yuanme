import type { MountContext } from "yuanme:mount";

/** What this Script uses of Pagefind's search API. */
interface Pagefind {
  options(options: { baseUrl: string }): Promise<void>;
  debouncedSearch(query: string): Promise<{ results: { data(): Promise<Result> }[] } | null>;
}

interface Result {
  url: string;
  meta: { title?: string };
  /** The page's text around the match, escaped, with each match in `<mark>`. */
  excerpt: string;
}

/**
 * Searches the docs with Pagefind, as the visitor types in the header's
 * search field. The deploy job runs Pagefind on the built Site
 * (site/assemble.ts), so its index exists only where the Site is deployed,
 * and its script loads only here, when there is something to search for.
 */
export function mount(ctx: MountContext) {
  const status = document.querySelector<HTMLElement>("#search-status");
  const list = document.querySelector<HTMLOListElement>("#results");
  const input = document.querySelector<HTMLInputElement>("#q");
  if (!status || !list || !input) return;
  // Built from parts, so the Engine's bundler leaves the import to the
  // browser: Pagefind's files aren't the project's.
  const bundle = `${status.dataset.base ?? "/"}pagefind/pagefind.js`;
  let pagefind: Promise<Pagefind> | undefined;
  async function load(): Promise<Pagefind> {
    const api: Pagefind = await import(bundle);
    // The index's URLs are from the root of the host, /next/ included;
    // Pagefind would otherwise put its own folder's parent before them.
    await api.options({ baseUrl: "/" });
    return api;
  }

  async function search(query: string) {
    if (!query.trim()) {
      list!.replaceChildren();
      status!.textContent = "";
      return;
    }
    let api: Pagefind;
    try {
      api = await (pagefind ??= load());
    } catch {
      status!.textContent = "Search isn't available here: its index is built when the site is deployed.";
      return;
    }
    const found = await api.debouncedSearch(query);
    // A later search replaced this one.
    if (found === null || ctx.signal.aborted) return;
    const results = await Promise.all(found.results.slice(0, 20).map((r) => r.data()));
    list!.replaceChildren(
      ...results.map((result) => {
        const item = document.createElement("li");
        const link = document.createElement("a");
        link.href = result.url;
        link.textContent = result.meta.title ?? result.url;
        const excerpt = document.createElement("p");
        excerpt.innerHTML = result.excerpt;
        item.append(link, excerpt);
        return item;
      }),
    );
    const n = found.results.length;
    status!.textContent = `${n} ${n === 1 ? "page" : "pages"} for “${query}”`;
  }

  const query = new URLSearchParams(location.search).get("q") ?? "";
  input.value = query;
  if (!query) input.focus();
  void search(query);
  input.addEventListener(
    "input",
    () => {
      const url = new URL(location.href);
      url.searchParams.set("q", input.value);
      history.replaceState(null, "", url);
      void search(input.value);
    },
    { signal: ctx.signal },
  );
}
