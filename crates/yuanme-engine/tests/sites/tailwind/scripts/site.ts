const fine = matchMedia("(pointer: fine)").matches;
document.querySelector("main")?.classList.toggle("ring-2", fine);
