import { endOfWeek, isClosed, isoDate, type Todo } from "../todo";

/** `late`, `today` or `week` for an open todo due by Sunday. */
export function dueClass(todo: Todo, today = isoDate(), sunday = endOfWeek()): string {
  if (!todo.due || isClosed(todo)) return "";
  if (todo.due < today) return "late";
  if (todo.due === today) return "today";
  if (todo.due <= sunday) return "week";
  return "";
}

interface Props {
  todo: Todo;
  /** Shown under the text, on pages that mix topics. */
  topic?: string | undefined;
  onToggle: (done: boolean) => void;
  onOpen: () => void;
}

/** One todo: its box, text, and what's known about it. */
export function TodoRow({ todo, topic, onToggle, onOpen }: Props) {
  const closed = isClosed(todo);
  return (
    <li className={`todo ${todo.status}${closed ? " closed" : ""}`}>
      <input
        type="checkbox"
        checked={closed}
        aria-label={`Done: ${todo.text}`}
        onChange={(event) => onToggle(event.target.checked)}
      />
      <button type="button" className="body" onClick={onOpen}>
        <span className="text">{todo.text}</span>
        <span className="meta">
          {todo.due && <span className={dueClass(todo)}>{todo.due}</span>}
          {todo.priority !== "normal" && (
            <span className={`prio-${todo.priority}`}>{todo.priority}</span>
          )}
          {todo.pinned && <span title="Pinned">📌</span>}
          {todo.recur && <span title="Repeats">🔁 {todo.recur.unit}</span>}
          {(todo.tags ?? []).map((tag) => (
            <span key={tag}>#{tag}</span>
          ))}
          {topic && <span>{topic}</span>}
        </span>
      </button>
    </li>
  );
}
