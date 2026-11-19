import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, test } from "vitest";
import { App, statusText } from "./App";
import { addTodo, emptyReplica } from "./replica";
import { KEYS, Store } from "./store";
import { isoDate } from "./todo";

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

function saved(text: string, due: string | null) {
  const replica = addTodo(
    emptyReplica(),
    {
      id: "a",
      text,
      note: "projects/demo",
      priority: "normal",
      due,
      tags: [],
      pinned: false,
      recur: null,
    },
    "todo",
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

  test("adding offline shows the todo and how many changes wait", async () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("buy milk", null) }),
      offline,
    );
    window.location.hash = "#/note/projects%2Fdemo";
    render(<App store={store} />);
    fireEvent.click(screen.getByRole("button", { name: "Add a todo" }));
    const form = screen.getByRole("dialog", { name: "Add" });
    fireEvent.change(within(form).getByLabelText("What"), { target: { value: "call mum" } });
    fireEvent.click(within(form).getByRole("button", { name: "Save" }));
    expect(screen.getByText("call mum")).toBeDefined();
    expect(await screen.findByText("offline · 1 to send")).toBeDefined();
  });

  test("ticking a todo moves it to done", () => {
    const store = new Store(
      storage({ [KEYS.token]: "t", [KEYS.replica]: saved("buy milk", null) }),
      offline,
    );
    window.location.hash = "#/note/projects%2Fdemo";
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
