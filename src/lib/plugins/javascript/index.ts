import type { Plugin } from "../types";

export const javascriptPlugin: Plugin = {
  id: "javascript",
  name: "JavaScript / TypeScript",
  version: "1.0.0",
  author: "Steroitor",
  description:
    "Evidenziazione con i template HTML/CSS inline (Angular, Lit), completamento, errori di sintassi (solo JavaScript) e Vai alla dichiarazione, anche dai tag dei componenti nei template HTML.",
  languages: [
    { id: "javascript", name: "JavaScript", fileExtensions: ["js", "mjs", "cjs"] },
    { id: "jsx", name: "JavaScript JSX", fileExtensions: ["jsx"] },
    { id: "typescript", name: "TypeScript", fileExtensions: ["ts", "mts", "cts"] },
    { id: "tsx", name: "TypeScript JSX", fileExtensions: ["tsx"] },
  ],
  load: () => import("./module"),
};
