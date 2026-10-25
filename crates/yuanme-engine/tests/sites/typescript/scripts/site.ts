import { greet } from "/scripts/greet";
import { count } from "./count.js";
import nav from "/data/nav.yaml";
import { navigate } from "yuanme:router";

const year: number = "2026";
greet(42);
const wave = "héllo 😀"; const loud: number = wave;
console.log(greet(BUILD_ID), count(nav), year, loud);

document.addEventListener("yuanme:before-swap", (event) => {
  const swap = event.swap;
  event.swap = () => {
    document.documentElement.dataset.leaving = event.from.pathname;
    swap();
  };
});
document.querySelector("h1")?.addEventListener("click", () => {
  navigate("/", { history: "replace" });
});
