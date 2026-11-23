// The app's state outside React: the copy, saved to localStorage after every
// change, plus how the last sync went. Components read it with `useStore`.

import { useSyncExternalStore } from "react";
import {
  addTodo,
  deleteTodo,
  emptyReplica,
  parseReplica,
  type Replica,
  updateTodo,
} from "./replica";
import { Api, sync } from "./sync";
import { isoDate, type NewTodo, type Status, type TodoPatch } from "./todo";

/** Where things are kept in the browser. */
export const KEYS = { token: "todo.token", replica: "todo.replica" } as const;

/** What the page shows. */
export interface State {
  replica: Replica;
  /** Why the last sync stopped: `offline`, `no token`, ... */
  problem: string | null;
  /** The last change the server refused. */
  refused: string | null;
  /** A sync is running. */
  syncing: boolean;
  /** The server token, empty until set. */
  token: string;
}

/** The parts of `Storage` the store uses, so tests can pass a fake. */
export type Storage = Pick<globalThis.Storage, "getItem" | "setItem">;

export class Store {
  #state: State;
  readonly #storage: Storage;
  readonly #fetch: typeof fetch | undefined;
  readonly #listeners = new Set<() => void>();
  #running: Promise<void> | null = null;
  #again = false;

  constructor(storage: Storage, fetchFn?: typeof fetch) {
    this.#storage = storage;
    this.#fetch = fetchFn;
    this.#state = {
      replica: parseReplica(read(storage, KEYS.replica)),
      problem: null,
      refused: null,
      syncing: false,
      token: read(storage, KEYS.token) ?? "",
    };
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  };

  readonly get = (): State => this.#state;

  #set(patch: Partial<State>): void {
    this.#state = { ...this.#state, ...patch };
    if (patch.replica) write(this.#storage, KEYS.replica, JSON.stringify(patch.replica));
    for (const listener of this.#listeners) listener();
  }

  /** A local change: shown and saved at once, then sent. */
  #change(replica: Replica): void {
    this.#set({ replica });
    void this.syncNow();
  }

  add(body: NewTodo, status: Status): void {
    this.#change(addTodo(this.#state.replica, body, status, isoDate()));
  }

  update(id: string, patch: TodoPatch): void {
    this.#change(updateTodo(this.#state.replica, id, patch, isoDate()));
  }

  remove(id: string): void {
    this.#change(deleteTodo(this.#state.replica, id));
  }

  setToken(token: string): void {
    write(this.#storage, KEYS.token, token);
    this.#set({ token });
    void this.syncNow();
  }

  /** Forgets the copy, queued changes included, and fetches everything. */
  reset(): void {
    this.#set({ replica: emptyReplica(), refused: null });
    void this.syncNow();
  }

  dismissRefused(): void {
    this.#set({ refused: null });
  }

  /** Runs a sync, or one more after the running one. */
  syncNow(): Promise<void> {
    if (this.#running) {
      this.#again = true;
      return this.#running;
    }
    this.#running = (async () => {
      this.#set({ syncing: true });
      do {
        this.#again = false;
        await this.#syncOnce();
      } while (this.#again);
      this.#running = null;
      this.#set({ syncing: false });
    })();
    return this.#running;
  }

  async #syncOnce(): Promise<void> {
    const { token } = this.#state;
    if (!token) {
      this.#set({ problem: "no token" });
      return;
    }
    const api = new Api(token, this.#fetch);
    const outcome = await sync(
      api,
      () => this.#state.replica,
      (replica) => this.#set({ replica }),
    );
    this.#set({ problem: outcome.problem, refused: outcome.refused ?? this.#state.refused });
  }
}

function read(storage: Storage, key: string): string | null {
  try {
    return storage.getItem(key);
  } catch {
    return null;
  }
}

function write(storage: Storage, key: string, value: string): void {
  try {
    storage.setItem(key, value);
  } catch (error) {
    console.warn(`could not save ${key}`, error);
  }
}

/** The state, re-rendering on every change. */
export function useStore(store: Store): State {
  return useSyncExternalStore(store.subscribe, store.get);
}
