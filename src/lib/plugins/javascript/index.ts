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
  options: [
    {
      id: "typeCheck",
      label: "Type-check con TypeScript",
      description:
        "Errori di tipo del file aperto, con il tsconfig del progetto e i tipi di node_modules. Gira in background: da ~50 MB a qualche centinaio di MB di RAM in più, secondo le dipendenze del progetto.",
    },
  ],
  load: (options) => import("./module").then((module) => module.create(options)),
};
