import { type FormEvent, useId, useState } from "react";
import type { Entry } from "../replica";
import {
  PRIORITIES,
  type Priority,
  type Status,
  type Todo,
  type TodoPatch,
  type Unit,
} from "../todo";

/** What the form edits. */
export interface Fields {
  text: string;
  note: string;
  due: string;
  priority: Priority;
  repeat: Unit | "";
  status: Status;
  tags: string;
  pinned: boolean;
}

/** The form's fields for `entry`, or for a new todo in `note`. */
export function fieldsOf(entry: Entry | null, note: string): Fields {
  const todo = entry?.todo;
  return {
    text: todo?.text ?? "",
    note: entry?.note ?? note,
    due: todo?.due ?? "",
    priority: todo?.priority ?? "normal",
    repeat: todo?.recur?.unit ?? "",
    status: todo?.status ?? "todo",
    tags: (todo?.tags ?? []).join(", "),
    pinned: Boolean(todo?.pinned),
  };
}

/** `home, #errands` as `["home", "errands"]`. */
export function parseTags(tags: string): string[] {
  return tags
    .split(/[\s,]+/)
    .map((tag) => tag.replace(/^#/, ""))
    .filter(Boolean);
}

/** Only what changed, so an edit doesn't undo one made elsewhere meanwhile. */
export function patchOf(todo: Todo, fields: Fields): TodoPatch {
  const patch: TodoPatch = {};
  const text = fields.text.trim();
  const due = fields.due || null;
  const recur = fields.repeat ? { every: todo.recur?.every ?? 1, unit: fields.repeat } : null;
  const tags = parseTags(fields.tags);
  if (text !== todo.text) patch.text = text;
  if (due !== (todo.due ?? null)) patch.due = due;
  if (fields.priority !== todo.priority) patch.priority = fields.priority;
  if (recur?.unit !== todo.recur?.unit) patch.recur = recur;
  if (tags.join(" ") !== (todo.tags ?? []).join(" ")) patch.tags = tags;
  if (fields.pinned !== Boolean(todo.pinned)) patch.pinned = fields.pinned;
  if (fields.status !== todo.status) patch.status = fields.status;
  return patch;
}

const STATUSES: [Status, string][] = [
  ["todo", "todo"],
  ["in_progress", "in progress"],
  ["done", "done"],
  ["cancelled", "cancelled"],
];
const REPEATS: [Unit | "", string][] = [
  ["", "never"],
  ["day", "every day"],
  ["week", "every week"],
  ["month", "every month"],
  ["year", "every year"],
];

interface Props {
  /** The todo to edit, or `null` to add one. */
  entry: Entry | null;
  /** Note a new todo goes in by default. */
  note: string;
  notes: string[];
  onSave: (fields: Fields) => void;
  onDelete: () => void;
  onClose: () => void;
}

/** Add or edit one todo, over the page. */
export function TodoForm({ entry, note, notes, onSave, onDelete, onClose }: Props) {
  const [fields, setFields] = useState(() => fieldsOf(entry, note));
  const [deleting, setDeleting] = useState(false);
  const list = useId();
  const set = <K extends keyof Fields>(key: K, value: Fields[K]) =>
    setFields((old) => ({ ...old, [key]: value }));
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!fields.text.trim() || !fields.note.trim()) return;
    onSave(fields);
  };
  return (
    <div className="backdrop">
      <form className="dialog" role="dialog" aria-label={entry ? "Edit" : "Add"} onSubmit={submit}>
        <h2>{entry ? "Edit" : "Add"}</h2>
        <label>
          What
          <input
            required
            autoComplete="off"
            value={fields.text}
            onChange={(e) => set("text", e.target.value)}
          />
        </label>
        <label>
          Topic
          <input
            required
            list={list}
            autoComplete="off"
            autoCapitalize="none"
            disabled={Boolean(entry)}
            value={fields.note}
            onChange={(e) => set("note", e.target.value)}
          />
        </label>
        <datalist id={list}>
          {notes.map((n) => (
            <option key={n} value={n} />
          ))}
        </datalist>
        <div className="row">
          <label>
            Due
            <input type="date" value={fields.due} onChange={(e) => set("due", e.target.value)} />
          </label>
          <label>
            Priority
            <select
              value={fields.priority}
              onChange={(e) => set("priority", e.target.value as Priority)}
            >
              {PRIORITIES.map((p) => (
                <option key={p} value={p}>
                  {p}
                </option>
              ))}
            </select>
          </label>
        </div>
        <div className="row">
          <label>
            Repeat
            <select
              value={fields.repeat}
              onChange={(e) => set("repeat", e.target.value as Unit | "")}
            >
              {REPEATS.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label>
            Status
            <select value={fields.status} onChange={(e) => set("status", e.target.value as Status)}>
              {STATUSES.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
        </div>
        <label>
          Tags
          <input
            placeholder="home, errands"
            autoComplete="off"
            autoCapitalize="none"
            value={fields.tags}
            onChange={(e) => set("tags", e.target.value)}
          />
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={fields.pinned}
            onChange={(e) => set("pinned", e.target.checked)}
          />
          Pinned
        </label>
        <div className="buttons">
          {entry && (
            <button
              type="button"
              className="danger"
              onClick={() => (deleting ? onDelete() : setDeleting(true))}
            >
              {deleting ? "Really delete?" : "Delete"}
            </button>
          )}
          <span className="grow" />
          <button type="button" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="primary">
            Save
          </button>
        </div>
      </form>
    </div>
  );
}
