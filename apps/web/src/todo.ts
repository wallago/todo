// The server's wire types (crates/client) and the rules the app applies
// locally the way the server does.

export type Status = "todo" | "in_progress" | "done" | "cancelled";
export type Priority = "highest" | "high" | "medium" | "normal" | "low" | "lowest";
export type Unit = "day" | "week" | "month" | "year";

/** Every priority, highest first. */
export const PRIORITIES: readonly Priority[] = [
  "highest",
  "high",
  "medium",
  "normal",
  "low",
  "lowest",
];

/** How often a todo comes back once done. */
export interface Recur {
  every: number;
  unit: Unit;
}

/** One todo, as the server sends it. Empty and default fields are left out. */
export interface Todo {
  id: string;
  text: string;
  status: Status;
  priority: Priority;
  created: string;
  due?: string | null;
  done?: string | null;
  tags?: string[];
  pinned?: boolean;
  recur?: Recur | null;
}

/** Body of `POST /api/todos`. */
export interface NewTodo {
  id: string;
  text: string;
  topic: string;
  priority: Priority;
  due: string | null;
  tags: string[];
  pinned: boolean;
  recur: Recur | null;
}

/** Body of `PATCH /api/todos/{id}`: fields left out stay; `null` clears. */
export interface TodoPatch {
  text?: string;
  status?: Status;
  priority?: Priority;
  due?: string | null;
  tags?: string[];
  pinned?: boolean;
  recur?: Recur | null;
}

/** A todo change in `GET /api/sync`: the todo plus its topic. */
export type TodoChange = Todo & { topic: string; deleted?: boolean };

/** Answer of `GET /api/sync?since=N`. */
export interface Changes {
  cursor: number;
  topics?: { name: string; deleted?: boolean }[];
  todos?: TodoChange[];
}

/** Local date as `YYYY-MM-DD`, `days` from `from`. */
export function isoDate(days = 0, from: Date = new Date()): string {
  const d = new Date(from);
  d.setDate(d.getDate() + days);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Sunday of the week of `from` (weeks start on Monday). */
export function endOfWeek(from: Date = new Date()): string {
  const monday0 = (from.getDay() + 6) % 7;
  return isoDate(6 - monday0, from);
}

/** Done or cancelled. */
export function isClosed(todo: Todo): boolean {
  return todo.status === "done" || todo.status === "cancelled";
}

/** Done (not cancelled) on `day`. */
export function isDoneOn(todo: Todo, day: string): boolean {
  return todo.status === "done" && todo.done === day;
}

/** How many todos were done on each day, newest day first. */
export function doneByDay(todos: Todo[]): { day: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const todo of todos)
    if (todo.status === "done" && todo.done)
      counts.set(todo.done, (counts.get(todo.done) ?? 0) + 1);
  return [...counts]
    .map(([day, count]) => ({ day, count }))
    .sort((a, b) => (a.day < b.day ? 1 : -1));
}

/** `Today`, `Yesterday`, or a short date like `Mon 23 Nov` for a `YYYY-MM-DD` day. */
export function dayLabel(day: string, today: string = isoDate()): string {
  if (day === today) return "Today";
  const date = new Date(`${day}T00:00`);
  if (isoDate(1, date) === today) return "Yesterday";
  const opts = { weekday: "short", day: "numeric", month: "short" } as const;
  return date.toLocaleDateString("en-GB", opts).replace(",", "");
}

/** The todo a new one starts as, like `NewTodo::to_todo`. */
export function newTodo(body: NewTodo, today: string): Todo {
  return {
    id: body.id,
    text: body.text.trim(),
    status: "todo",
    priority: body.priority,
    created: today,
    due: body.due,
    done: null,
    tags: body.tags,
    pinned: body.pinned,
    recur: body.recur,
  };
}

/** Applies `patch` to `todo`, like `TodoPatch::apply` on the server. */
export function applyPatch(todo: Todo, patch: TodoPatch, today: string): Todo {
  const next: Todo = { ...todo };
  if (patch.text !== undefined) next.text = patch.text.trim();
  if (patch.priority !== undefined) next.priority = patch.priority;
  if (patch.due !== undefined) next.due = patch.due;
  if (patch.tags !== undefined) next.tags = patch.tags;
  if (patch.pinned !== undefined) next.pinned = patch.pinned;
  if (patch.recur !== undefined) next.recur = patch.recur;
  if (patch.status === "done") {
    if (todo.status !== "done") {
      next.status = "done";
      next.done = today;
    }
  } else if (patch.status !== undefined) {
    next.status = patch.status;
    next.done = null;
  }
  return next;
}

/** Pinned first, then by due date (none last), then priority. */
export function byUrgency(a: Todo, b: Todo): number {
  return (
    Number(Boolean(b.pinned)) - Number(Boolean(a.pinned)) ||
    (a.due ?? "9999").localeCompare(b.due ?? "9999") ||
    PRIORITIES.indexOf(a.priority) - PRIORITIES.indexOf(b.priority)
  );
}

/** How many todos there are, by state, as the TUI dashboard counts them. */
export interface Counts {
  total: number;
  /** Done or cancelled. */
  done: number;
  open: number;
  /** Open and due before today. */
  late: number;
  /** Open and due today. */
  today: number;
}

/** Counts `todos` by state. */
export function countTodos(todos: Todo[], today = isoDate()): Counts {
  const counts = { total: todos.length, done: 0, open: 0, late: 0, today: 0 };
  for (const todo of todos) {
    if (isClosed(todo)) {
      counts.done++;
      continue;
    }
    counts.open++;
    if (todo.due && todo.due < today) counts.late++;
    if (todo.due === today) counts.today++;
  }
  return counts;
}
