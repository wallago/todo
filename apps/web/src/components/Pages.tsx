import type { Entry } from "../replica";
import { topicHref } from "../route";
import { byUrgency, countTodos, endOfWeek, isClosed, isDoneOn, isoDate } from "../todo";
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

/** Link to the done-today page, with how many todos were done today. */
function DoneTodayLink({ count }: { count: number }) {
  return (
    <a className="done-today" href="#/done-today">
      <span className={count > 0 ? "done" : undefined}>✓ {count}</span> done today
    </a>
  );
}

/** Late, today, this week, then later: every open todo with a due date. */
export function DuePage({ todos, ...list }: PageProps) {
  const today = isoDate();
  const doneToday = todos.filter((e) => isDoneOn(e.todo, today)).length;
  const sunday = endOfWeek();
  const open = todos.filter((e) => !isClosed(e.todo)).sort((a, b) => byUrgency(a.todo, b.todo));
  const sections: [string, Entry[]][] = [
    ["Late", open.filter((e) => e.todo.due && e.todo.due < today)],
    ["Today", open.filter((e) => e.todo.due === today)],
    ["This week", open.filter((e) => e.todo.due && e.todo.due > today && e.todo.due <= sunday)],
    ["Later", open.filter((e) => e.todo.due && e.todo.due > sunday)],
  ];
  const shown = sections.filter(([, entries]) => entries.length > 0);
  if (todos.length === 0) return <p className="empty">No todos yet.</p>;
  return (
    <>
      <DoneTodayLink count={doneToday} />
      {shown.length === 0 && <p className="empty">Nothing due.</p>}
      {shown.map(([title, entries]) => (
        <section key={title}>
          <h2>
            {title} ({entries.length})
          </h2>
          <TodoList entries={entries} showTopic {...list} />
        </section>
      ))}
    </>
  );
}

/** The todos done today, across topics; unticking one puts it back. */
export function DoneTodayPage({ todos, ...list }: PageProps) {
  const today = isoDate();
  const done = todos.filter((e) => isDoneOn(e.todo, today));
  return (
    <>
      <h1>Done today ({done.length})</h1>
      {done.length ? (
        <TodoList entries={done} showTopic {...list} />
      ) : (
        <p className="empty">Nothing done yet today.</p>
      )}
    </>
  );
}

/** Every open pinned todo, across topics. */
export function PinnedPage({ todos, ...list }: PageProps) {
  const pinned = todos
    .filter((e) => e.todo.pinned && !isClosed(e.todo))
    .sort((a, b) => byUrgency(a.todo, b.todo));
  if (pinned.length === 0) return <p className="empty">Nothing pinned.</p>;
  return <TodoList entries={pinned} showTopic {...list} />;
}

/** A bar filled to `done` of `total`. */
function Progress({ done, total }: { done: number; total: number }) {
  return (
    <span className="progress" aria-hidden="true">
      <span style={{ width: `${total ? (100 * done) / total : 0}%` }} />
    </span>
  );
}

/** One number of the summary card. */
function Tile({ label, value, tone }: { label: string; value: number; tone?: string }) {
  return (
    <div className="tile">
      <span>{label}</span>
      <strong className={value > 0 ? tone : undefined}>{value}</strong>
    </div>
  );
}

/** A card of counts over every todo, then each topic with its own. */
export function TopicsPage({ topics, todos }: { topics: string[]; todos: Entry[] }) {
  if (topics.length === 0) return <p className="empty">No topics yet.</p>;
  const all = countTodos(todos.map((e) => e.todo));
  return (
    <>
      <section className="summary" aria-label="Summary">
        <div className="tiles">
          <Tile label="Late" value={all.late} tone="late" />
          <Tile label="Today" value={all.today} tone="today" />
          <Tile label="Open" value={all.open} />
          <Tile label="Done" value={all.done} tone="done" />
        </div>
        <Progress done={all.done} total={all.total} />
        <span className="ratio">
          {all.done}/{all.total} done
        </span>
      </section>
      <ul>
        {topics.map((topic) => {
          const counts = countTodos(todos.filter((e) => e.topic === topic).map((e) => e.todo));
          return (
            <li key={topic} className="topic">
              <a href={topicHref(topic)}>
                <span className="name">{topic}</span>
                <span className="count">
                  {counts.open} open
                  {counts.late > 0 && <span className="late"> · {counts.late} late</span>}
                </span>
                <Progress done={counts.done} total={counts.total} />
              </a>
            </li>
          );
        })}
      </ul>
    </>
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
