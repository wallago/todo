import { describe, expect, test } from "vitest";
import { addTodo, emptyReplica, merge, type Replica, updateTodo } from "./replica";
import { Api, sync } from "./sync";
import type { Changes } from "./todo";

const TODAY = "2026-11-19";

/** A fake server: answers by method and path, records every call. */
function server(answer: (method: string, path: string) => Response | Promise<Response>) {
  const calls: string[] = [];
  const fetchFn = async (input: RequestInfo | URL, init?: RequestInit) => {
    const call = `${init?.method ?? "GET"} ${String(input)}`;
    calls.push(call);
    return answer(init?.method ?? "GET", String(input));
  };
  return { calls, api: new Api("secret", fetchFn as typeof fetch) };
}

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });

function holder(start: Replica) {
  let replica = start;
  return {
    get: () => replica,
    set: (next: Replica) => {
      replica = next;
    },
  };
}

const milk = {
  id: "11111111-1111-4111-8111-111111111111",
  text: "milk",
  topic: "errands",
  priority: "normal" as const,
  due: null,
  tags: [],
  pinned: false,
  recur: null,
};

describe("sync", () => {
  test("sends the queue in order, then pulls the changes", async () => {
    let replica = addTodo(emptyReplica(), milk, "todo", TODAY);
    replica = updateTodo(replica, milk.id, { status: "done" }, TODAY);
    const changes: Changes = {
      cursor: 7,
      todos: replica.todos.map((e) => ({ ...e.todo, topic: e.topic })),
    };
    const { calls, api } = server((_method, path) =>
      path.startsWith("/api/sync") ? json(200, changes) : json(200, {}),
    );
    const state = holder(replica);
    const outcome = await sync(api, state.get, state.set);
    expect(outcome).toEqual({ problem: null, refused: null });
    expect(calls).toEqual([
      "POST /api/todos",
      `PATCH /api/todos/${milk.id}`,
      "GET /api/sync?since=0",
    ]);
    expect(state.get().outbox).toEqual([]);
    expect(state.get().cursor).toBe(7);
  });

  test("offline keeps the queue", async () => {
    const { api } = server(() => Promise.reject(new TypeError("Failed to fetch")));
    const state = holder(addTodo(emptyReplica(), milk, "todo", TODAY));
    expect(await sync(api, state.get, state.set)).toEqual({ problem: "offline", refused: null });
    expect(state.get().outbox).toHaveLength(1);
  });

  test("a bad token keeps the queue and says so", async () => {
    const { api } = server(() => json(401, { error: "missing or wrong bearer token" }));
    const state = holder(addTodo(emptyReplica(), milk, "todo", TODAY));
    expect((await sync(api, state.get, state.set)).problem).toBe("token refused");
    expect(state.get().outbox).toHaveLength(1);
  });

  test("a refused change is dropped, reported, and undone by a full sync", async () => {
    const start = merge(
      emptyReplica(),
      {
        cursor: 3,
        todos: [
          { id: "a", text: "a", status: "todo", priority: "normal", created: TODAY, topic: "n" },
        ],
      },
      0,
    );
    const replica = updateTodo(start, "a", { text: "b" }, TODAY);
    const { calls, api } = server((method) => {
      if (method === "PATCH") return json(404, { error: "no todo with id `a`" });
      return json(200, { cursor: 3, todos: [] });
    });
    const state = holder(replica);
    const outcome = await sync(api, state.get, state.set);
    expect(outcome.refused).toBe('edit "b": 404: no todo with id `a`');
    expect(calls.at(-1)).toBe("GET /api/sync?since=0");
    expect(state.get().todos).toEqual([]);
  });

  test("deleting a todo already gone counts as sent", async () => {
    const { api } = server((method) =>
      method === "DELETE" ? json(404, { error: "gone" }) : json(200, { cursor: 1 }),
    );
    const state = holder({
      ...emptyReplica(),
      outbox: [{ seq: 0, op: { kind: "delete", id: "x" } }],
      nextSeq: 1,
    });
    expect(await sync(api, state.get, state.set)).toEqual({ problem: null, refused: null });
    expect(state.get().outbox).toEqual([]);
  });

  test("a cursor going back fetches everything again", async () => {
    const { calls, api } = server(() => json(200, { cursor: 2 }));
    const state = holder({ ...emptyReplica(), cursor: 9 });
    await sync(api, state.get, state.set);
    expect(calls).toEqual(["GET /api/sync?since=9", "GET /api/sync?since=0"]);
  });

  test("sends the token", async () => {
    let auth: string | null = null;
    const api = new Api("secret", (async (_input: RequestInfo | URL, init?: RequestInit) => {
      auth = new Headers(init?.headers).get("Authorization");
      return json(200, { cursor: 0 });
    }) as typeof fetch);
    await api.changes(0);
    expect(auth).toBe("Bearer secret");
  });
});
