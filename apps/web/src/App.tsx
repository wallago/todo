import { useCallback, useEffect, useState } from "react";
import { updateBadge } from "./badge";
import { BottomBar, Drawer, GearButton } from "./components/Nav";
import { DoneTodayPage, DuePage, PinnedPage, TopicPage, TopicsPage } from "./components/Pages";
import { type Fields, parseTags, patchOf, TodoForm } from "./components/TodoForm";
import { TokenForm } from "./components/TokenForm";
import type { Entry } from "./replica";
import { TABS, tabOf, useRoute } from "./route";
import { type Store, useStore } from "./store";
import { useSwipe } from "./swipe";
import { isoDate } from "./todo";

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
  const [drawer, setDrawer] = useState(false);
  useAutoSync(store);
  // Every sync and local change makes a new copy, so the icon follows both.
  useEffect(() => void updateBadge(replica.todos, isoDate()), [replica]);
  const tab = tabOf(route);
  useSwipe(
    (step) => {
      const next = TABS[tab + step];
      if (next) window.location.hash = next.href;
    },
    editing === undefined && !settings && !drawer,
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
          topic: fields.topic.trim().replace(/\.md$/, ""),
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
  const closeDrawer = useCallback(() => setDrawer(false), []);
  const status = statusText(problem, replica.outbox.length, refused);

  return (
    <>
      <header>
        <GearButton onClick={() => setDrawer(true)} />
        <span className="title">{TABS[tab]?.label}</span>
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
      </header>
      <main>
        {route.page === "topic" ? (
          <TopicPage topic={route.topic} {...list} />
        ) : route.page === "done-today" ? (
          <DoneTodayPage {...list} />
        ) : route.page === "pinned" ? (
          <PinnedPage {...list} />
        ) : route.page === "topics" ? (
          <TopicsPage topics={replica.topics} todos={replica.todos} />
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
      <BottomBar active={tab} />
      {drawer && (
        <Drawer status={status} onSettings={() => setSettings(true)} onClose={closeDrawer} />
      )}
      {editing !== undefined && (
        <TodoForm
          entry={editing}
          topic={route.page === "topic" ? route.topic : (replica.topics[0] ?? "")}
          topics={replica.topics}
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
          onNotify={() => void updateBadge(replica.todos, isoDate())}
        />
      )}
    </>
  );
}
