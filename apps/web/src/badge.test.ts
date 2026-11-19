import { expect, test, vi } from "vitest";
import { type Badging, dueCount, showBadge } from "./badge";
import type { Entry } from "./replica";
import type { Todo } from "./todo";

function entry(due: string | null, status: Todo["status"] = "todo"): Entry {
  return {
    note: "main",
    todo: {
      id: crypto.randomUUID(),
      text: "t",
      status,
      priority: "normal",
      created: "2026-11-01",
      due,
    },
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
});

function badging(): Badging & {
  setAppBadge: ReturnType<typeof vi.fn>;
  clearAppBadge: ReturnType<typeof vi.fn>;
} {
  return {
    setAppBadge: vi.fn(() => Promise.resolve()),
    clearAppBadge: vi.fn(() => Promise.resolve()),
  };
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
