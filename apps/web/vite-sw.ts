import type { Plugin } from "vite";

/** The worker's source. */
const SOURCE = "sw.ts";
/** Where the build writes it: at the site root, so its scope is the whole site. */
const OUTPUT = "sw.js";

/**
 * Builds `sw.ts` to `sw.js` next to `index.html`, and writes into it the list
 * of every other built file to precache. Build only: `vite` dev has no worker.
 */
export function serviceWorker(): Plugin {
  return {
    name: "service-worker",
    apply: "build",
    config: () => ({
      build: {
        rolldownOptions: {
          input: { index: "index.html", sw: SOURCE },
          output: {
            entryFileNames: (chunk) => (chunk.name === "sw" ? OUTPUT : "assets/[name]-[hash].js"),
          },
        },
      },
    }),
    generateBundle(_options, bundle) {
      const worker = bundle[OUTPUT];
      if (worker?.type !== "chunk") {
        this.error(`the build wrote no ${OUTPUT}`);
      }
      // The page itself is `./`: that's the URL a visit asks for.
      const files = [
        "./",
        ...Object.keys(bundle).filter(
          (file) => file !== OUTPUT && file !== "index.html" && !file.endsWith(".map"),
        ),
      ];
      worker.code = worker.code
        .replaceAll("__PRECACHE__", JSON.stringify(files))
        .replaceAll("__VERSION__", JSON.stringify(hash(files.join("\n"))));
    },
  };
}

/** A short FNV-1a hash of `text`: the cache's name changes with any file. */
function hash(text: string): string {
  let h = 0x811c9dc5;
  for (let at = 0; at < text.length; at++) {
    h = Math.imul(h ^ text.charCodeAt(at), 0x01000193);
  }
  return (h >>> 0).toString(36);
}
