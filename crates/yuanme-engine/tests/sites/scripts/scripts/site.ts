import { navigate } from "yuanme:router";
import { id } from "tiny-id";
import film from "/data/film.toml";

export const palette: string[] = film.palette;

document.addEventListener("click", () => navigate("/about/", { history: "push" }));
console.log(id());

// The chunk it loads imports this one back.
const lazy = () => import("./lazy.ts");
document.addEventListener("dblclick", async () => (await lazy()).default());
