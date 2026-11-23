import { expect, test } from "vitest";
import { countTodos, type Todo } from "./todo";

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
