// The local copy of every todo, and the changes not sent yet: the same model
// as the TUI's replica (apps/tui/src/tui/replica.rs).

import { applyPatch, type Changes, type NewTodo, newTodo, type Todo, type TodoPatch } from "./todo";

/** A change made here, waiting to reach the server. */
export type Op =
  | { kind: "add"; body: NewTodo }
  | { kind: "update"; id: string; patch: TodoPatch }
  | { kind: "delete"; id: string };

/** One queued change, numbered so it can be acknowledged. */
export interface Pending {
  seq: number;
  op: Op;
}

/** One todo and the note it's in. */
export interface Entry {
  note: string;
  todo: Todo;
}

/** The copy. Plain data, so it saves as JSON; every change returns a new one. */
export interface Replica {
  /** `cursor` of the last sync. */
  cursor: number;
  /** The server refused a change: the next sync fetches everything. */
  resync: boolean;
  /** Every note name, sorted. */
  notes: string[];
  /** Every todo, in the order they arrived. */
  todos: Entry[];
  /** Changes not sent yet, oldest first. */
  outbox: Pending[];
  /** Number of the next queued change. */
  nextSeq: number;
}

/** An empty copy. */
export function emptyReplica(): Replica {
  return { cursor: 0, resync: false, notes: [], todos: [], outbox: [], nextSeq: 0 };
}

/** The todo an op is about. */
export function opId(op: Op): string {
  return op.kind === "add" ? op.body.id : op.id;
}

function queue(replica: Replica, op: Op): Replica {
  return {
    ...replica,
    outbox: [...replica.outbox, { seq: replica.nextSeq, op }],
    nextSeq: replica.nextSeq + 1,
  };
}

/** Adds `body` to its note and queues it; a status other than todo is an edit after. */
export function addTodo(
  replica: Replica,
  body: NewTodo,
  status: Todo["status"],
  today: string,
): Replica {
  const notes = replica.notes.includes(body.note)
    ? replica.notes
    : [...replica.notes, body.note].sort();
  let next = queue(
    {
      ...replica,
      notes,
      todos: [...replica.todos, { note: body.note, todo: newTodo(body, today) }],
    },
    { kind: "add", body },
  );
  if (status !== "todo") {
    next = updateTodo(next, body.id, { status }, today);
  }
  return next;
}

/** Applies `patch` to todo `id` and queues it; unchanged if there's no such todo. */
export function updateTodo(replica: Replica, id: string, patch: TodoPatch, today: string): Replica {
  if (!replica.todos.some((entry) => entry.todo.id === id)) return replica;
  const todos = replica.todos.map((entry) =>
    entry.todo.id === id ? { ...entry, todo: applyPatch(entry.todo, patch, today) } : entry,
  );
  return queue({ ...replica, todos }, { kind: "update", id, patch });
}

/** Removes todo `id` and queues it. */
export function deleteTodo(replica: Replica, id: string): Replica {
  const todos = replica.todos.filter((entry) => entry.todo.id !== id);
  return queue({ ...replica, todos }, { kind: "delete", id });
}

/** Change `seq` reached the server, or was refused (then `resync`). */
export function settle(replica: Replica, seq: number, refused: boolean): Replica {
  return {
    ...replica,
    outbox: replica.outbox.filter((pending) => pending.seq !== seq),
    resync: replica.resync || refused,
  };
}

/** What the next sync asks for. */
export function since(replica: Replica): number {
  return replica.resync ? 0 : replica.cursor;
}

/**
 * Takes in what changed on the server since `from` (0: everything). Todos
 * with a change still queued keep theirs: the server sends them again once
 * it has that change.
 */
export function merge(replica: Replica, changes: Changes, from: number): Replica {
  const waiting = new Set(replica.outbox.map((pending) => opId(pending.op)));
  let todos = replica.todos;
  let notes = new Set(replica.notes);
  let resync = replica.resync;
  if (from === 0) {
    todos = todos.filter((entry) => waiting.has(entry.todo.id));
    notes = new Set(todos.map((entry) => entry.note));
    resync = false;
  }
  for (const note of changes.notes ?? []) {
    if (note.deleted) notes.delete(note.name);
    else notes.add(note.name);
  }
  todos = [...todos];
  for (const { note, deleted, ...todo } of changes.todos ?? []) {
    if (waiting.has(todo.id)) continue;
    const at = todos.findIndex((entry) => entry.todo.id === todo.id);
    if (deleted) {
      if (at >= 0) todos.splice(at, 1);
    } else if (at >= 0) {
      todos[at] = { note, todo };
    } else {
      todos.push({ note, todo });
    }
  }
  return { ...replica, todos, notes: [...notes].sort(), cursor: changes.cursor, resync };
}

/** Reads a saved copy, or an empty one if it's missing or unreadable. */
export function parseReplica(saved: string | null): Replica {
  if (saved === null) return emptyReplica();
  try {
    const parsed: unknown = JSON.parse(saved);
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      "todos" in parsed &&
      "outbox" in parsed &&
      Array.isArray(parsed.todos) &&
      Array.isArray(parsed.outbox)
    ) {
      return { ...emptyReplica(), ...(parsed as Partial<Replica>) };
    }
  } catch {
    // Unreadable: start over.
  }
  return emptyReplica();
}
