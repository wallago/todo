import type { Entry } from "../replica";
import { topicHref } from "../route";
import { byUrgency, endOfWeek, isClosed, isoDate } from "../todo";
import { TodoRow } from "./TodoRow";

interface ListProps {
  entries: Entry[];
  showTopic: boolean;
  onToggle: (id: string, done: boolean) => void;
  onOpen: (entry: Entry) => void;
}

function TodoList({ entries, showTopic, onToggle, onOpen }: ListProps) {
  return (
    <ul>
      {entries.map((entry) => (
        <TodoRow
          key={entry.todo.id}
          todo={entry.todo}
          topic={showTopic ? entry.topic : undefined}
          onToggle={(done) => onToggle(entry.todo.id, done)}
          onOpen={() => onOpen(entry)}
        />
      ))}
    </ul>
  );
}

type PageProps = Omit<ListProps, "entries" | "showTopic"> & { todos: Entry[] };

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
      <TodoList entries={entries} showTopic {...list} />
    </section>
  ));
}

/** Every open pinned todo, across topics. */
export function PinnedPage({ todos, ...list }: PageProps) {
  const pinned = todos
    .filter((e) => e.todo.pinned && !isClosed(e.todo))
    .sort((a, b) => byUrgency(a.todo, b.todo));
  if (pinned.length === 0) return <p className="empty">Nothing pinned.</p>;
  return <TodoList entries={pinned} showTopic {...list} />;
}

/** Every topic, with how many todos are still open in it. */
export function TopicsPage({ topics, todos }: { topics: string[]; todos: Entry[] }) {
  if (topics.length === 0) return <p className="empty">No topics yet.</p>;
  return (
    <ul>
      {topics.map((topic) => {
        const open = todos.filter((e) => e.topic === topic && !isClosed(e.todo)).length;
        return (
          <li key={topic} className="topic">
            <a href={topicHref(topic)}>
              <span>{topic}</span>
              <span className="count">{open} open</span>
            </a>
          </li>
        );
      })}
    </ul>
  );
}

/** One topic: open todos first, done ones folded below. */
export function TopicPage({ topic, todos, ...list }: PageProps & { topic: string }) {
  const mine = todos.filter((e) => e.topic === topic);
  const open = mine.filter((e) => !isClosed(e.todo)).sort((a, b) => byUrgency(a.todo, b.todo));
  const done = mine.filter((e) => isClosed(e.todo));
  return (
    <>
      <h1>{topic}</h1>
      {open.length ? (
        <TodoList entries={open} showTopic={false} {...list} />
      ) : (
        <p className="empty">Nothing open.</p>
      )}
      {done.length > 0 && (
        <details>
          <summary>Done ({done.length})</summary>
          <TodoList entries={done} showTopic={false} {...list} />
        </details>
      )}
    </>
  );
}
