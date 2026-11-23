import { expect, test, vi } from "vitest";
import { type Badging, dueCount, dueTodos, showBadge, showDue } from "./badge";
import type { Entry } from "./replica";
import type { Todo } from "./todo";

function entry(due: string | null, status: Todo["status"] = "todo", text = "t"): Entry {
  return {
    topic: "main",
    todo: { id: crypto.randomUUID(), text, status, priority: "normal", created: "2026-11-01", due },
  };
}

test("counts open todos due today or late", () => {
  const todos = [
    entry("2026-11-18"),
    entry("2026-11-19"),
    entry("2026-11-19", "in_progress"),
    entry("2026-11-20"),
    entry(null),
    entry("2026-11-19", "done"),
    entry("2026-11-10", "cancelled"),
  ];
  expect(dueCount(todos, "2026-11-19")).toBe(3);
  expect(dueTodos(todos, "2026-11-19")).toHaveLength(3);
});

function badging() {
  return {
    setAppBadge: vi.fn(() => Promise.resolve()),
    clearAppBadge: vi.fn(() => Promise.resolve()),
  } satisfies Badging;
}

test("shows the count on the icon", async () => {
  const nav = badging();
  await showBadge(4, nav);
  expect(nav.setAppBadge).toHaveBeenCalledWith(4);
});

test("clears the icon when nothing is due", async () => {
  const nav = badging();
  await showBadge(0, nav);
  expect(nav.clearAppBadge).toHaveBeenCalled();
  expect(nav.setAppBadge).not.toHaveBeenCalled();
});

test("skips a browser without badges", async () => {
  await expect(showBadge(2, {})).resolves.toBeUndefined();
});

test("ignores a refused badge", async () => {
  const nav = badging();
  nav.setAppBadge.mockRejectedValue(new DOMException("denied", "NotAllowedError"));
  await expect(showBadge(1, nav)).resolves.toBeUndefined();
});

/** A fake registration holding its notifications by tag. */
function notifier(tags: string[] = []) {
  const shown = new Map<string, { title: string; body: string; close: () => void }>();
  for (const tag of tags)
    shown.set(tag, { title: "old", body: "old", close: () => shown.delete(tag) });
  const fake = {
    shown,
    getNotifications: vi.fn(async () =>
      [...shown.entries()].map(([tag, n]) => ({
        tag,
        title: n.title,
        body: n.body,
        close: n.close,
      })),
    ),
    showNotification: vi.fn(async (title: string, options?: NotificationOptions) => {
      const tag = options?.tag ?? "";
      shown.set(tag, { title, body: options?.body ?? "", close: () => shown.delete(tag) });
    }),
  };
  return fake;
}

test("posts one quiet notification per due todo", async () => {
  const late = entry("2026-11-12", "todo", "Make an example");
  const today = entry("2026-11-20", "todo", "Change sync");
  const reg = notifier();
  await showDue([late, today, entry("2026-11-21")], "2026-11-20", reg);
  expect([...reg.shown.keys()].sort()).toEqual(
    [`due:${late.todo.id}`, `due:${today.todo.id}`].sort(),
  );
  expect(reg.shown.get(`due:${late.todo.id}`)).toMatchObject({
    title: "Make an example",
    body: "late · 2026-11-12 · main",
  });
  expect(reg.showNotification).toHaveBeenCalledWith(
    "Change sync",
    expect.objectContaining({ silent: true }),
  );
});

test("closes the notification of a todo no longer due, and leaves others' alone", async () => {
  const kept = entry("2026-11-20");
  const reg = notifier([`due:${kept.todo.id}`, "due:gone", "other"]);
  reg.shown.set(`due:${kept.todo.id}`, { title: "t", body: "today · main", close: () => {} });
  await showDue([kept], "2026-11-20", reg);
  expect([...reg.shown.keys()].sort()).toEqual([`due:${kept.todo.id}`, "other"].sort());
  // Unchanged: not posted again, so it doesn't move or buzz.
  expect(reg.showNotification).not.toHaveBeenCalled();
});

test("skips notifications without a registration or permission", async () => {
  await expect(showDue([entry("2026-11-20")], "2026-11-20", undefined)).resolves.toBeUndefined();
  const reg = notifier();
  reg.showNotification.mockRejectedValue(new TypeError("no permission"));
  await expect(showDue([entry("2026-11-20")], "2026-11-20", reg)).resolves.toBeUndefined();
});
