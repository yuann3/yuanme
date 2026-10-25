// A Site-level Script: once per document, it starts the persisted canvas,
// as the Reference site's Film does, and keeps the persisted nav in step
// through the Router's events.
import { navigate } from "yuanme:router";

const w = window as any;
w.__navigate = navigate;
w.__events = [];
w.__navErrors = 0;
for (const name of ["before-swap", "after-swap", "page-load"]) {
  document.addEventListener(`yuanme:${name}`, () => w.__events.push(name));
}
(window as any).navigation?.addEventListener("navigateerror", () => w.__navErrors++);

const canvas = document.querySelector<HTMLCanvasElement>("[data-film-canvas]")!;
(canvas as any).__probeId = Math.random();
if (canvas.getContext("webgl") ?? canvas.getContext("2d")) {
  sessionStorage.setItem("__ctx", String(Number(sessionStorage.getItem("__ctx") ?? 0) + 1));
}

const sync = () => {
  for (const a of document.querySelectorAll<HTMLAnchorElement>("[data-nav-link]")) {
    const here = location.pathname === a.pathname;
    here ? a.setAttribute("aria-current", "page") : a.removeAttribute("aria-current");
  }
};
sync();
document.addEventListener("yuanme:page-load", sync);
