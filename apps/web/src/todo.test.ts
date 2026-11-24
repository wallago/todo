import { expect, test } from "vitest";
import { countTodos, isDoneOn, type Todo } from "./todo";

function todo(status: Todo["status"], due: string | null = null): Todo {
  return { id: "x", text: "x", status, priority: "normal", created: "2026-11-01", due };
}

test("counts open, done, late and due today", () => {
  const todos = [
    todo("todo", "2026-11-19"),
    todo("in_progress", "2026-11-20"),
    todo("todo"),
    todo("done", "2026-11-01"),
    todo("cancelled"),
  ];
  expect(countTodos(todos, "2026-11-20")).toEqual({
    total: 5,
    done: 2,
    open: 3,
    late: 1,
    today: 1,
  });
});

test("isDoneOn needs status done and that done date", () => {
  const day = "2026-11-24";
  expect(isDoneOn({ ...todo("done"), done: day }, day)).toBe(true);
  expect(isDoneOn({ ...todo("done"), done: "2026-11-23" }, day)).toBe(false);
  expect(isDoneOn({ ...todo("cancelled"), done: day }, day)).toBe(false);
  expect(isDoneOn(todo("todo"), day)).toBe(false);
});
