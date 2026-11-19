import { useSyncExternalStore } from "react";

/** A page of the app, from the URL hash. */
export type Route = { page: "due" } | { page: "notes" } | { page: "note"; note: string };

/** Reads `#/`, `#/notes` or `#/note/<name>`. */
export function parseRoute(hash: string): Route {
  const path = hash.replace(/^#\/?/, "");
  if (path.startsWith("note/")) return { page: "note", note: decodeURIComponent(path.slice(5)) };
  if (path === "notes") return { page: "notes" };
  return { page: "due" };
}

/** The hash of a note's page. */
export function noteHref(note: string): string {
  return `#/note/${encodeURIComponent(note)}`;
}

function subscribe(listener: () => void): () => void {
  window.addEventListener("hashchange", listener);
  return () => window.removeEventListener("hashchange", listener);
}

/** The current page, following hash changes. */
export function useRoute(): Route {
  const hash = useSyncExternalStore(subscribe, () => window.location.hash);
  return parseRoute(hash);
}
