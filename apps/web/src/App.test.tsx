import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, test } from "vitest";
import { App, statusText } from "./App";
import { addTodo, emptyReplica } from "./replica";
import { KEYS, Store } from "./store";
import { isoDate, type Status } from "./todo";

/** In-memory `localStorage`. */
function storage(items: Record<string, string> = {}) {
  const map = new Map(Object.entries(items));
  return {
    map,
    getItem: (key: string) => map.get(key) ?? null,
    setItem: (key: string, value: string) => void map.set(key, value),
  };
}

/** A server that is down. */
const offline = (() => Promise.reject(new TypeError("Failed to fetch"))) as typeof fetch;

function saved(text: string, due: string | null, pinned = false, status: Status = "todo") {
  const replica = addTodo(
    emptyReplica(),
    {
      id: "a",
      text,
      topic: "projects/demo",
      priority: "normal",
      due,
      tags: [],
      pinned,
      recur: null,
    },
    status,
    isoDate(),
  );
  return JSON.stringify({ ...replica, outbox: [] });
}

beforeEach(() => {
  window.location.hash = "";
});

describe("App", () => {
  test("asks for the token first", () => {
    render(<App store={new Store(storage(), offline)} />);
    expect(screen.getByRole("dialog", { name: "Server token" })).toBeDefined();
  });

  test("shows what's due today from the saved copy", () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("buy milk", isoDate()) }),
      offline,
    );
    render(<App store={store} />);
    expect(screen.getByRole("heading", { name: "Today (1)" })).toBeDefined();
    expect(screen.getByText("buy milk")).toBeDefined();
  });

  test("todos due after this week show under Later", () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("renew passport", isoDate(30)) }),
      offline,
    );
    render(<App store={store} />);
    expect(screen.getByRole("heading", { name: "Later (1)" })).toBeDefined();
    expect(screen.getByText("renew passport")).toBeDefined();
  });

  test("adding offline shows the todo and how many changes wait", async () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("buy milk", null) }),
      offline,
    );
    window.location.hash = "#/topic/projects%2Fdemo";
    render(<App store={store} />);
    fireEvent.click(screen.getByRole("button", { name: "Add a todo" }));
    const form = screen.getByRole("dialog", { name: "Add" });
    fireEvent.change(within(form).getByLabelText("What"), { target: { value: "call mum" } });
    fireEvent.click(within(form).getByRole("button", { name: "Save" }));
    expect(screen.getByText("call mum")).toBeDefined();
    expect(await screen.findByText("offline · 1 to send")).toBeDefined();
  });

  test("pinned todos get their own page, not a Due section", async () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("water plants", null, true) }),
      offline,
    );
    render(<App store={store} />);
    expect(screen.getByText("Nothing due.")).toBeDefined();
    expect(screen.getByRole("link", { name: "Pinned" }).getAttribute("href")).toBe("#/pinned");
    window.location.hash = "#/pinned";
    expect(await screen.findByText("water plants")).toBeDefined();
  });

  test("swiping moves between Due, Pinned and Topics", async () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("buy milk", null) }),
      offline,
    );
    render(<App store={store} />);
    // Swipes from `from` to `to`, then waits for the tab that should be active.
    const swipe = async (from: number, to: number, active: string) => {
      fireEvent.touchStart(document, { touches: [{ clientX: from, clientY: 200 }] });
      fireEvent.touchEnd(document, { changedTouches: [{ clientX: to, clientY: 210 }] });
      await waitFor(() =>
        expect(screen.getByRole("link", { name: active }).className).toBe("active"),
      );
    };
    await swipe(300, 100, "Pinned");
    await swipe(300, 100, "Topics");
    await swipe(300, 100, "Topics");
    await swipe(100, 300, "Pinned");
    await swipe(200, 220, "Pinned");
    await swipe(100, 300, "Due");
  });

  test("the Topics page sums up every topic", () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("buy milk", isoDate(-1)) }),
      offline,
    );
    window.location.hash = "#/topics";
    render(<App store={store} />);
    const summary = screen.getByRole("region", { name: "Summary" });
    expect(within(summary).getByText("Late").nextSibling?.textContent).toBe("1");
    expect(within(summary).getByText("Open").nextSibling?.textContent).toBe("1");
    expect(within(summary).getByText("0/1 done")).toBeDefined();
    const topic = screen.getByRole("link", { name: /projects\/demo/ });
    expect(topic.querySelector(".count")?.textContent).toBe("1 open · 1 late");
  });

  test("a due date can be cleared", async () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("buy milk", isoDate()) }),
      offline,
    );
    render(<App store={store} />);
    fireEvent.click(screen.getByText("buy milk"));
    const form = screen.getByRole("dialog", { name: "Edit" });
    fireEvent.click(within(form).getByRole("button", { name: "Clear due date" }));
    expect((within(form).getByLabelText("Due") as HTMLInputElement).value).toBe("");
    fireEvent.click(within(form).getByRole("button", { name: "Save" }));
    expect(screen.getByText("Nothing due.")).toBeDefined();
    expect(await screen.findByText("offline · 1 to send")).toBeDefined();
  });

  test("the Due page counts todos done today and lists them on their own page", async () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("buy milk", isoDate(), false, "done") }),
      offline,
    );
    render(<App store={store} />);
    expect(screen.getByText("Nothing due.")).toBeDefined();
    const link = screen.getByRole("link", { name: "✓ 1 done today" });
    expect(link.getAttribute("href")).toBe("#/done-today");
    window.location.hash = "#/done-today";
    expect(await screen.findByRole("heading", { name: "Done today (1)" })).toBeDefined();
    expect(screen.getByText("buy milk")).toBeDefined();
    expect(screen.getByRole("link", { name: "Due" }).className).toBe("active");
    fireEvent.click(screen.getByRole("checkbox", { name: "Done: buy milk" }));
    expect(screen.getByText("Nothing done yet today.")).toBeDefined();
  });

  test("the gear opens a drawer with Done today and Settings", async () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("buy milk", null) }),
      offline,
    );
    render(<App store={store} />);
    expect(screen.getByRole("navigation", { name: "Pages" })).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Menu" }));
    const drawer = screen.getByRole("complementary", { name: "Menu" });
    expect(within(drawer).getByRole("link", { name: "Done today" }).getAttribute("href")).toBe(
      "#/done-today",
    );
    fireEvent.click(screen.getByRole("button", { name: "Close menu" }));
    expect(screen.queryByRole("complementary", { name: "Menu" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Menu" }));
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.queryByRole("complementary", { name: "Menu" })).toBeNull();
    expect(screen.getByRole("dialog", { name: "Server token" })).toBeDefined();
  });

  test("ticking a todo moves it to done", () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("buy milk", null) }),
      offline,
    );
    window.location.hash = "#/topic/projects%2Fdemo";
    render(<App store={store} />);
    fireEvent.click(screen.getByRole("checkbox", { name: "Done: buy milk" }));
    expect(screen.getByText("Done (1)")).toBeDefined();
    expect(screen.getByText("Nothing open.")).toBeDefined();
  });
});

test("status text", () => {
  expect(statusText(null, 0, null)).toBe("");
  expect(statusText("offline", 3, null)).toBe("offline · 3 to send");
  expect(statusText(null, 2, null)).toBe("2 to send");
  expect(statusText("offline", 2, "edit x: 404")).toBe("refused: edit x: 404");
});
