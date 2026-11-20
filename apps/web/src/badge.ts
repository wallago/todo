// The number on the installed app's icon: open todos due today or late,
// counted from the local copy. Desktop Chrome and Edge take the number
// through the Badging API. Android has no such API: its launcher badges an
// app by its notifications, so each due todo also gets a quiet notification.

import type { Entry } from "./replica";
import { isClosed } from "./todo";

/** Tag prefix of the due notifications, so other notifications are left alone. */
const TAG = "due:";

/** Open todos due on or before `today` (`YYYY-MM-DD`). */
export function dueTodos(todos: readonly Entry[], today: string): Entry[] {
  return todos.filter((e) => !isClosed(e.todo) && e.todo.due && e.todo.due <= today);
}

/** How many open todos are due on or before `today`. */
export function dueCount(todos: readonly Entry[], today: string): number {
  return dueTodos(todos, today).length;
}

/** What badging needs of `navigator`: absent where the browser has no badges. */
export type Badging = Partial<Pick<Navigator, "setAppBadge" | "clearAppBadge">>;

/** Shows `count` on the app icon, or clears it at 0. Never throws. */
export async function showBadge(count: number, nav: Badging = navigator): Promise<void> {
  try {
    if (count > 0) await nav.setAppBadge?.(count);
    else await nav.clearAppBadge?.();
  } catch {
    // Not installed, or badges turned off: the icon just shows no number.
  }
}

/** A notification as `getNotifications` gives it back. */
interface Shown {
  tag: string;
  title: string;
  body: string;
  close(): void;
}

/** What notifying needs of the service worker's registration. */
export interface Notifier {
  getNotifications(): Promise<Shown[]>;
  showNotification(title: string, options?: NotificationOptions): Promise<void>;
}

/** The notification's second line: `late · 2026-11-12 · work/misc`. */
function bodyOf(entry: Entry, today: string): string {
  const when = entry.todo.due === today ? "today" : `late · ${entry.todo.due}`;
  return `${when} · ${entry.topic}`;
}

/**
 * One quiet notification per todo due today or late, closed once it isn't.
 * An unchanged one is left as is, so it doesn't move or buzz. Does nothing
 * without a registration or notification permission. Never throws.
 */
export async function showDue(
  todos: readonly Entry[],
  today: string,
  reg: Notifier | undefined,
): Promise<void> {
  if (!reg) return;
  try {
    const due = new Map(dueTodos(todos, today).map((e) => [`${TAG}${e.todo.id}`, e]));
    const shown = (await reg.getNotifications()).filter((n) => n.tag.startsWith(TAG));
    for (const n of shown) {
      if (!due.has(n.tag)) n.close();
    }
    for (const [tag, entry] of due) {
      const body = bodyOf(entry, today);
      const old = shown.find((n) => n.tag === tag);
      if (old?.title === entry.todo.text && old.body === body) continue;
      await reg.showNotification(entry.todo.text, {
        tag,
        body,
        silent: true,
        icon: `${import.meta.env.BASE_URL}icons/icon-192.png`,
        data: { id: entry.todo.id },
      });
    }
  } catch {
    // No permission, or notifications turned off: the icon shows no badge.
  }
}

/** The worker's registration once notifications are allowed, else nothing. */
export async function notifier(): Promise<Notifier | undefined> {
  if (typeof Notification === "undefined" || Notification.permission !== "granted")
    return undefined;
  if (!("serviceWorker" in navigator)) return undefined;
  return navigator.serviceWorker.getRegistration();
}

/** Puts the count on the icon, both ways. */
export async function updateBadge(todos: readonly Entry[], today: string): Promise<void> {
  await showBadge(dueCount(todos, today));
  await showDue(todos, today, await notifier());
}
