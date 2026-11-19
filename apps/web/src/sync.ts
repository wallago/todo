// Talking to todo-server: send the queued changes oldest first, then pull
// `GET /api/sync?since=cursor`, like the TUI does.

import { merge, type Op, type Replica, settle, since } from "./replica";
import type { Changes } from "./todo";

/** A failed call. `refused`: sending it again won't help, so drop it. */
export class ApiError extends Error {
  readonly status: number;
  readonly refused: boolean;

  constructor(status: number, message: string) {
    super(`${status}: ${message}`);
    this.status = status;
    this.refused = status >= 400 && status < 500 && ![401, 403, 408, 429].includes(status);
  }
}

/** Calls the server's API with the bearer token. */
export class Api {
  readonly #token: string;
  readonly #fetch: typeof fetch;

  constructor(token: string, fetchFn: typeof fetch = (...args) => fetch(...args)) {
    this.#token = token;
    this.#fetch = fetchFn;
  }

  async call<T>(method: string, path: string, body?: unknown): Promise<T | null> {
    const headers: Record<string, string> = { Authorization: `Bearer ${this.#token}` };
    const init: RequestInit = { method, headers, cache: "no-store" };
    if (body !== undefined) {
      headers["Content-Type"] = "application/json";
      init.body = JSON.stringify(body);
    }
    const response = await this.#fetch(path, init);
    if (!response.ok) {
      let message = response.statusText;
      try {
        const answer: unknown = await response.json();
        if (typeof answer === "object" && answer !== null && "error" in answer) {
          message = String(answer.error);
        }
      } catch {
        // Not JSON: keep the status text.
      }
      throw new ApiError(response.status, message);
    }
    return response.status === 204 ? null : ((await response.json()) as T);
  }

  /** Sends one queued change. Deleting a todo already gone is fine. */
  async send(op: Op): Promise<void> {
    if (op.kind === "add") {
      await this.call("POST", "/api/todos", op.body);
      return;
    }
    const path = `/api/todos/${encodeURIComponent(op.id)}`;
    if (op.kind === "update") {
      await this.call("PATCH", path, op.patch);
      return;
    }
    try {
      await this.call("DELETE", path);
    } catch (error) {
      if (!(error instanceof ApiError && error.status === 404)) throw error;
    }
  }

  async changes(from: number): Promise<Changes> {
    const changes = await this.call<Changes>("GET", `/api/sync?since=${from}`);
    if (changes === null) throw new ApiError(204, "no changes in the answer");
    return changes;
  }
}

/** How a sync ended. */
export interface Outcome {
  /** Why it stopped early: `offline`, `token refused`, or the server's answer. */
  problem: string | null;
  /** The last change the server refused, now dropped. */
  refused: string | null;
}

/** Short description of a change, for a refused one. */
export function describe(op: Op, replica: Replica): string {
  if (op.kind === "add") return `add "${op.body.text}"`;
  const entry = replica.todos.find((e) => e.todo.id === op.id);
  const what = entry ? `"${entry.todo.text}"` : op.id;
  return `${op.kind === "update" ? "edit" : "delete"} ${what}`;
}

/**
 * One sync. Reads the copy through `get` after every call, so changes made
 * meanwhile are kept, and writes it back through `set` after each step.
 */
export async function sync(
  api: Api,
  get: () => Replica,
  set: (replica: Replica) => void,
): Promise<Outcome> {
  let refused: string | null = null;
  try {
    for (let pending = get().outbox[0]; pending; pending = get().outbox[0]) {
      let wasRefused = false;
      try {
        await api.send(pending.op);
      } catch (error) {
        if (!(error instanceof ApiError && error.refused)) throw error;
        refused = `${describe(pending.op, get())}: ${error.message}`;
        wasRefused = true;
      }
      set(settle(get(), pending.seq, wasRefused));
    }
    let from = since(get());
    let changes = await api.changes(from);
    // A cursor going back means a new database: start over.
    if (changes.cursor < from) {
      from = 0;
      changes = await api.changes(0);
    }
    set(merge(get(), changes, from));
    return { problem: null, refused };
  } catch (error) {
    if (error instanceof ApiError) {
      return { problem: error.status === 401 ? "token refused" : error.message, refused };
    }
    return { problem: "offline", refused };
  }
}
