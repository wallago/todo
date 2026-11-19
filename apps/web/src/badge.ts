// The number on the installed app's icon: open todos due today or late,
// counted from the local copy, through the Badging API where there is one.

import type { Entry } from "./replica";
import { isClosed } from "./todo";

/** Open todos due on or before `today` (`YYYY-MM-DD`). */
export function dueCount(todos: readonly Entry[], today: string): number {
  return todos.filter((e) => !isClosed(e.todo) && e.todo.due && e.todo.due <= today).length;
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
