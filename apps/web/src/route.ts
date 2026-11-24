import { useSyncExternalStore } from "react";

/** A page of the app, from the URL hash. */
export type Route =
  | { page: "due" }
  | { page: "done-today" }
  | { page: "pinned" }
  | { page: "topics" }
  | { page: "topic"; topic: string };

/** The pages of the tab bar, in swipe order. */
export const TABS = [
  { page: "due", href: "#/", label: "Due" },
  { page: "pinned", href: "#/pinned", label: "Pinned" },
  { page: "topics", href: "#/topics", label: "Topics" },
] as const;

/** Reads `#/`, `#/done-today`, `#/pinned`, `#/topics` or `#/topic/<name>`. */
export function parseRoute(hash: string): Route {
  const path = hash.replace(/^#\/?/, "");
  if (path.startsWith("topic/"))
    return { page: "topic", topic: decodeURIComponent(path.slice("topic/".length)) };
  if (path === "topics") return { page: "topics" };
  if (path === "pinned") return { page: "pinned" };
  if (path === "done-today") return { page: "done-today" };
  return { page: "due" };
}

/** The tab a page sits under: a topic's page is under Topics, done today under Due. */
export function tabOf(route: Route): number {
  const page = route.page === "topic" ? "topics" : route.page === "done-today" ? "due" : route.page;
  return TABS.findIndex((tab) => tab.page === page);
}

/** The hash of a topic's page. */
export function topicHref(topic: string): string {
  return `#/topic/${encodeURIComponent(topic)}`;
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
