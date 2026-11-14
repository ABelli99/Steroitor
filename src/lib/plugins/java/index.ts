import type { Plugin } from "../types";

export const javaPlugin: Plugin = {
  id: "java",
  name: "Java",
  version: "1.1.0",
  author: "Steroitor",
  description: "Completamento per Java. Disattivato, resta l'evidenziazione.",
  languages: [{ id: "java", name: "Java", fileExtensions: ["java"] }],
  options: [
    {
      id: "completion",
      label: "Completamento",
      description:
        "Senza JVM: variabili, campi, metodi e classi del file aperto, membri dopo il punto con il tipo restituito, le classi più usate del JDK con documentazione, parole chiave e snippet (sout, main, fori, foreach).",
      default: true,
    },
    {
      id: "jdtls",
      label: "Completamento completo con jdtls (language server)",
      description:
        "Classi, metodi e campi di tutto il progetto e delle dipendenze Maven/Gradle, con Javadoc e import automatico. Serve il Completamento attivo. Usa jdtls e Java 21+ dall'estensione Java di VS Code (redhat.java) o da JDTLS_HOME / JDTLS_JAVA_HOME. Da ~300 MB a oltre 1 GB di RAM; il primo avvio su un progetto importa le dipendenze e può richiedere qualche minuto.",
    },
  ],
  load: (options) => import("./module").then((module) => module.create(options)),
};
