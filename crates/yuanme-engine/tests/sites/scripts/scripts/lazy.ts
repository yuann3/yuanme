import { greet } from "./shared";
import { palette } from "./site.ts";

export default (): void => greet(palette[0]);
