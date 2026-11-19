import type { Entry } from "../replica";
import { noteHref } from "../route";
import { byUrgency, endOfWeek, isClosed, isoDate } from "../todo";
import { TodoRow } from "./TodoRow";

interface ListProps {
  entries: Entry[];
  showNote: boolean;
  onToggle: (id: string, done: boolean) => void;
  onOpen: (entry: Entry) => void;
}

function TodoList({ entries, showNote, onToggle, onOpen }: ListProps) {
  return (
    <ul>
      {entries.map((entry) => (
        <TodoRow
          key={entry.todo.id}
          todo={entry.todo}
          note={showNote ? entry.note : undefined}
          onToggle={(done) => onToggle(entry.todo.id, done)}
          onOpen={() => onOpen(entry)}
        />
      ))}
    </ul>
  );
}

type PageProps = Omit<ListProps, "entries" | "showNote"> & { todos: Entry[] };

/** Late, today, then this week: every open todo due by Sunday. */
export function DuePage({ todos, ...list }: PageProps) {
  const today = isoDate();
  const sunday = endOfWeek();
  const open = todos.filter((e) => !isClosed(e.todo)).sort((a, b) => byUrgency(a.todo, b.todo));
  const sections: [string, Entry[]][] = [
    ["Late", open.filter((e) => e.todo.due && e.todo.due < today)],
    ["Today", open.filter((e) => e.todo.due === today)],
    ["This week", open.filter((e) => e.todo.due && e.todo.due > today && e.todo.due <= sunday)],
  ];
  const shown = sections.filter(([, entries]) => entries.length > 0);
  if (shown.length === 0) {
    return <p className="empty">{todos.length ? "Nothing due this week." : "No todos yet."}</p>;
  }
  return shown.map(([title, entries]) => (
    <section key={title}>
      <h2>
        {title} ({entries.length})
      </h2>
      <TodoList entries={entries} showNote {...list} />
    </section>
  ));
}

/** Every open pinned todo, across notes. */
export function PinnedPage({ todos, ...list }: PageProps) {
  const pinned = todos
    .filter((e) => e.todo.pinned && !isClosed(e.todo))
    .sort((a, b) => byUrgency(a.todo, b.todo));
  if (pinned.length === 0) return <p className="empty">Nothing pinned.</p>;
  return <TodoList entries={pinned} showNote {...list} />;
}

/** Every note, with how many todos are still open in it. */
export function NotesPage({ notes, todos }: { notes: string[]; todos: Entry[] }) {
  if (notes.length === 0) return <p className="empty">No notes yet.</p>;
  return (
    <ul>
      {notes.map((note) => {
        const open = todos.filter((e) => e.note === note && !isClosed(e.todo)).length;
        return (
          <li key={note} className="note">
            <a href={noteHref(note)}>
              <span>{note}</span>
              <span className="count">{open} open</span>
            </a>
          </li>
        );
      })}
    </ul>
  );
}

/** One note: open todos first, done ones folded below. */
export function NotePage({ note, todos, ...list }: PageProps & { note: string }) {
  const mine = todos.filter((e) => e.note === note);
  const open = mine.filter((e) => !isClosed(e.todo)).sort((a, b) => byUrgency(a.todo, b.todo));
  const done = mine.filter((e) => isClosed(e.todo));
  return (
    <>
      <h1>{note}</h1>
      {open.length ? (
        <TodoList entries={open} showNote={false} {...list} />
      ) : (
        <p className="empty">Nothing open.</p>
      )}
      {done.length > 0 && (
        <details>
          <summary>Done ({done.length})</summary>
          <TodoList entries={done} showNote={false} {...list} />
        </details>
      )}
    </>
  );
}
