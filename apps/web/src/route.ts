import { useSyncExternalStore } from "react";

/** A page of the app, from the URL hash. */
export type Route =
  | { page: "due" }
  | { page: "pinned" }
  | { page: "notes" }
  | { page: "note"; note: string };

/** The pages of the tab bar, in swipe order. */
export const TABS = [
  { page: "due", href: "#/", label: "Due" },
  { page: "pinned", href: "#/pinned", label: "Pinned" },
  { page: "notes", href: "#/notes", label: "Notes" },
] as const;

/** Reads `#/`, `#/pinned`, `#/notes` or `#/note/<name>`. */
export function parseRoute(hash: string): Route {
  const path = hash.replace(/^#\/?/, "");
  if (path.startsWith("note/")) return { page: "note", note: decodeURIComponent(path.slice(5)) };
  if (path === "notes") return { page: "notes" };
  if (path === "pinned") return { page: "pinned" };
  return { page: "due" };
}

/** The tab a page sits under: a note's page is under Notes. */
export function tabOf(route: Route): number {
  const page = route.page === "note" ? "notes" : route.page;
  return TABS.findIndex((tab) => tab.page === page);
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
