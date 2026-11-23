import { describe, expect, test } from "vitest";
import {
  addTodo,
  deleteTodo,
  emptyReplica,
  merge,
  parseReplica,
  settle,
  since,
  updateTodo,
} from "./replica";
import type { NewTodo, Todo } from "./todo";

const TODAY = "2026-11-19";

function body(text: string, id = text): NewTodo {
  return {
    id,
    text,
    topic: "errands",
    priority: "normal",
    due: null,
    tags: [],
    pinned: false,
    recur: null,
  };
}

function todo(id: string, text = id): Todo {
  return { id, text, status: "todo", priority: "normal", created: TODAY };
}

describe("local changes", () => {
  test("an add shows at once, in its topic, and is queued", () => {
    const replica = addTodo(emptyReplica(), body("milk"), "todo", TODAY);
    expect(replica.todos).toEqual([
      { topic: "errands", todo: expect.objectContaining({ id: "milk", text: "milk" }) },
    ]);
    expect(replica.topics).toEqual(["errands"]);
    expect(replica.outbox).toEqual([{ seq: 0, op: { kind: "add", body: body("milk") } }]);
  });

  test("an add with another status queues an edit after it", () => {
    const replica = addTodo(emptyReplica(), body("milk"), "done", TODAY);
    expect(replica.todos[0]?.todo).toMatchObject({ status: "done", done: TODAY });
    expect(replica.outbox.map((p) => p.op.kind)).toEqual(["add", "update"]);
  });

  test("edits and deletes apply here, then queue", () => {
    let replica = addTodo(emptyReplica(), body("milk"), "todo", TODAY);
    replica = updateTodo(replica, "milk", { text: " oat milk ", pinned: true }, TODAY);
    expect(replica.todos[0]?.todo).toMatchObject({ text: "oat milk", pinned: true });
    replica = deleteTodo(replica, "milk");
    expect(replica.todos).toEqual([]);
    expect(replica.outbox).toHaveLength(3);
  });

  test("an edit of an unknown todo changes nothing", () => {
    const replica = emptyReplica();
    expect(updateTodo(replica, "gone", { text: "x" }, TODAY)).toBe(replica);
  });

  test("ticking stamps the done date, reopening clears it", () => {
    let replica = addTodo(emptyReplica(), body("milk"), "todo", TODAY);
    replica = updateTodo(replica, "milk", { status: "done" }, TODAY);
    expect(replica.todos[0]?.todo).toMatchObject({ status: "done", done: TODAY });
    replica = updateTodo(replica, "milk", { status: "todo" }, TODAY);
    expect(replica.todos[0]?.todo).toMatchObject({ status: "todo", done: null });
  });
});

describe("sync", () => {
  test("merge adds, changes and removes", () => {
    let replica = merge(
      emptyReplica(),
      {
        cursor: 2,
        topics: [{ name: "home" }, { name: "errands" }],
        todos: [
          { ...todo("a"), topic: "errands" },
          { ...todo("b"), topic: "home" },
        ],
      },
      0,
    );
    expect(replica.topics).toEqual(["errands", "home"]);
    expect(since(replica)).toBe(2);
    replica = merge(
      replica,
      {
        cursor: 3,
        topics: [{ name: "home", deleted: true }],
        todos: [
          { ...todo("b"), topic: "home", deleted: true },
          { ...todo("a", "eggs"), topic: "errands" },
        ],
      },
      2,
    );
    expect(replica.todos).toEqual([{ topic: "errands", todo: todo("a", "eggs") }]);
    expect(replica.topics).toEqual(["errands"]);
  });

  test("a todo with a queued change keeps its local version", () => {
    const local = addTodo(emptyReplica(), body("milk"), "todo", TODAY);
    const replica = merge(
      local,
      { cursor: 5, todos: [{ ...todo("milk", "stale"), topic: "errands" }] },
      0,
    );
    expect(replica.todos.map((e) => e.todo.text)).toEqual(["milk"]);
    expect(replica.topics).toEqual(["errands"]);
  });

  test("a refused change is undone by the next full sync", () => {
    let replica = merge(emptyReplica(), { cursor: 4 }, 0);
    replica = addTodo(replica, body("milk"), "todo", TODAY);
    replica = settle(replica, 0, true);
    expect(since(replica)).toBe(0);
    replica = merge(replica, { cursor: 4 }, 0);
    expect(replica.todos).toEqual([]);
    expect(replica.topics).toEqual([]);
    expect(since(replica)).toBe(4);
  });
});

describe("saved copy", () => {
  test("reads back what was saved", () => {
    const replica = addTodo(emptyReplica(), body("milk"), "todo", TODAY);
    expect(parseReplica(JSON.stringify(replica))).toEqual(replica);
  });

  test("a copy from before topics keeps its outbox and fetches the rest", () => {
    const { outbox, nextSeq } = addTodo(emptyReplica(), body("milk"), "todo", TODAY);
    const old = { cursor: 7, resync: false, notes: ["a"], todos: [{ note: "a" }], outbox, nextSeq };
    expect(parseReplica(JSON.stringify(old))).toEqual({
      ...emptyReplica(),
      outbox,
      nextSeq,
      resync: true,
    });
  });

  test("starts empty when there is none or it's unreadable", () => {
    expect(parseReplica(null)).toEqual(emptyReplica());
    expect(parseReplica("{not json")).toEqual(emptyReplica());
    expect(parseReplica('{"todos": 3}')).toEqual(emptyReplica());
  });
});
