import { greet } from "./shared";

export function welcome(page: string): void {
  greet(`the ${page} page`);
}
