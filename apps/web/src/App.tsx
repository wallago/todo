import { useEffect, useState } from "react";
import { DuePage, NotePage, NotesPage, PinnedPage } from "./components/Pages";
import { type Fields, parseTags, patchOf, TodoForm } from "./components/TodoForm";
import { TokenForm } from "./components/TokenForm";
import type { Entry } from "./replica";
import { TABS, tabOf, useRoute } from "./route";
import { type Store, useStore } from "./store";
import { useSwipe } from "./swipe";

/** How often the app syncs on its own while it's on screen. */
const AUTO_SYNC_MS = 30_000;

/** `offline · 3 to send`, or the last refused change. */
export function statusText(problem: string | null, pending: number, refused: string | null) {
  if (refused) return `refused: ${refused}`;
  return [problem, pending > 0 ? `${pending} to send` : null].filter(Boolean).join(" · ");
}

/** Syncs now, every 30 s while visible, on coming back online or on screen. */
function useAutoSync(store: Store) {
  useEffect(() => {
    const visible = () => document.visibilityState === "visible";
    const now = () => void store.syncNow();
    const onVisible = () => visible() && now();
    now();
    const timer = window.setInterval(onVisible, AUTO_SYNC_MS);
    window.addEventListener("online", now);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("online", now);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [store]);
}

/** The app: header, the page the URL names, and the add/edit form. */
export function App({ store }: { store: Store }) {
  const { replica, problem, refused, syncing, token } = useStore(store);
  const route = useRoute();
  // `null`: adding a todo; an entry: editing it; `undefined`: closed.
  const [editing, setEditing] = useState<Entry | null | undefined>(undefined);
  const [settings, setSettings] = useState(!token);
  useAutoSync(store);
  const tab = tabOf(route);
  useSwipe(
    (step) => {
      const next = TABS[tab + step];
      if (next) window.location.hash = next.href;
    },
    editing === undefined && !settings,
  );

  const list = {
    todos: replica.todos,
    onToggle: (id: string, done: boolean) => store.update(id, { status: done ? "done" : "todo" }),
    onOpen: (entry: Entry) => setEditing(entry),
  };
  const save = (fields: Fields) => {
    if (editing) {
      const patch = patchOf(editing.todo, fields);
      if (Object.keys(patch).length > 0) store.update(editing.todo.id, patch);
    } else {
      store.add(
        {
          id: crypto.randomUUID(),
          text: fields.text.trim(),
          note: fields.note.trim().replace(/\.md$/, ""),
          priority: fields.priority,
          due: fields.due || null,
          tags: parseTags(fields.tags),
          pinned: fields.pinned,
          recur: fields.repeat ? { every: 1, unit: fields.repeat } : null,
        },
        fields.status,
      );
    }
    setEditing(undefined);
  };
  const status = statusText(problem, replica.outbox.length, refused);

  return (
    <>
      <header>
        <nav>
          {TABS.map(({ href, label }, i) => (
            <a key={href} href={href} className={i === tab ? "active" : ""}>
              {label}
            </a>
          ))}
        </nav>
        <output className={refused || problem === "token refused" ? "status error" : "status"}>
          {status}
        </output>
        <button
          type="button"
          className={syncing ? "icon spin" : "icon"}
          aria-label="Sync now"
          onClick={() => {
            store.dismissRefused();
            void store.syncNow();
          }}
        >
          ⟳
        </button>
        <button
          type="button"
          className="icon"
          aria-label="Settings"
          onClick={() => setSettings(true)}
        >
          ⚙
        </button>
      </header>
      <main>
        {route.page === "note" ? (
          <NotePage note={route.note} {...list} />
        ) : route.page === "pinned" ? (
          <PinnedPage {...list} />
        ) : route.page === "notes" ? (
          <NotesPage notes={replica.notes} todos={replica.todos} />
        ) : (
          <DuePage {...list} />
        )}
      </main>
      <button
        type="button"
        className="add"
        aria-label="Add a todo"
        onClick={() => setEditing(null)}
      >
        +
      </button>
      {editing !== undefined && (
        <TodoForm
          entry={editing}
          note={route.page === "note" ? route.note : (replica.notes[0] ?? "")}
          notes={replica.notes}
          onSave={save}
          onDelete={() => {
            if (editing) store.remove(editing.todo.id);
            setEditing(undefined);
          }}
          onClose={() => setEditing(undefined)}
        />
      )}
      {settings && (
        <TokenForm
          token={token}
          pending={replica.outbox.length}
          onSave={(value) => {
            store.setToken(value);
            setSettings(false);
          }}
          onReset={() => {
            store.reset();
            setSettings(false);
          }}
          onClose={() => setSettings(false)}
        />
      )}
    </>
  );
}
