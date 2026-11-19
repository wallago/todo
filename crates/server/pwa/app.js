// Todo PWA: a local copy of every todo plus a queue of changes, kept in
// localStorage, synced with todo-server like the TUI does: send the queue
// oldest first, then pull GET /api/sync?since=cursor.
"use strict";

const TOKEN_KEY = "todo.token";
const REPLICA_KEY = "todo.replica";
const AUTO_SYNC_MS = 30_000;

// ---- Dates ----------------------------------------------------------------

/** Local date as YYYY-MM-DD, offset by `days`. */
function isoDate(days = 0) {
  const d = new Date();
  d.setDate(d.getDate() + days);
  const pad = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Sunday of this week (weeks start Monday), as YYYY-MM-DD. */
function endOfWeek() {
  const day = (new Date().getDay() + 6) % 7; // Monday 0
  return isoDate(6 - day);
}

// ---- Replica --------------------------------------------------------------

/** A fresh, empty copy. */
function emptyReplica() {
  return { cursor: 0, resync: false, notes: [], todos: [], outbox: [], nextSeq: 0 };
}

function loadReplica() {
  try {
    const saved = JSON.parse(localStorage.getItem(REPLICA_KEY));
    if (saved && Array.isArray(saved.todos) && Array.isArray(saved.outbox)) return saved;
  } catch (error) {
    console.warn("local copy unreadable, starting over", error);
  }
  return emptyReplica();
}

let replica = loadReplica();

function save() {
  try {
    localStorage.setItem(REPLICA_KEY, JSON.stringify(replica));
  } catch (error) {
    console.warn("could not save the local copy", error);
  }
}

function find(id) {
  return replica.todos.find((entry) => entry.todo.id === id);
}

function queue(op) {
  replica.outbox.push({ seq: replica.nextSeq++, op });
}

function opId(op) {
  return op.kind === "add" ? op.body.id : op.id;
}

/** Same as the server's TodoPatch::apply. */
function applyPatch(todo, patch) {
  for (const key of ["text", "priority", "due", "tags", "pinned", "recur"]) {
    if (key in patch) todo[key] = patch[key];
  }
  if (typeof todo.text === "string") todo.text = todo.text.trim();
  if (patch.status === "done") {
    if (todo.status !== "done") {
      todo.status = "done";
      todo.done = isoDate();
    }
  } else if (patch.status) {
    todo.status = patch.status;
    todo.done = null;
  }
}

/** Adds a todo locally and queues it; `status` other than todo is an edit after. */
function addTodo(fields, status) {
  const body = {
    id: crypto.randomUUID(),
    text: fields.text.trim(),
    note: fields.note,
    priority: fields.priority,
    due: fields.due,
    tags: fields.tags,
    pinned: fields.pinned,
    recur: fields.recur,
  };
  const todo = {
    id: body.id,
    text: body.text,
    status: "todo",
    priority: body.priority,
    due: body.due,
    created: isoDate(),
    done: null,
    tags: body.tags,
    pinned: body.pinned,
    recur: body.recur,
  };
  replica.todos.push({ note: body.note, todo });
  if (!replica.notes.includes(body.note)) {
    replica.notes.push(body.note);
    replica.notes.sort();
  }
  queue({ kind: "add", body });
  if (status !== "todo") updateTodo(body.id, { status });
  else changed();
}

function updateTodo(id, patch) {
  const entry = find(id);
  if (!entry) return;
  applyPatch(entry.todo, patch);
  queue({ kind: "update", id, patch });
  changed();
}

function deleteTodo(id) {
  replica.todos = replica.todos.filter((entry) => entry.todo.id !== id);
  queue({ kind: "delete", id });
  changed();
}

/** Takes in GET /api/sync's answer. Todos with a queued change keep theirs. */
function merge(changes, since) {
  const waiting = new Set(replica.outbox.map((pending) => opId(pending.op)));
  if (since === 0) {
    replica.todos = replica.todos.filter((entry) => waiting.has(entry.todo.id));
    replica.notes = [...new Set(replica.todos.map((entry) => entry.note))];
    replica.resync = false;
  }
  const notes = new Set(replica.notes);
  for (const note of changes.notes || []) {
    if (note.deleted) notes.delete(note.name);
    else notes.add(note.name);
  }
  replica.notes = [...notes].sort();
  for (const change of changes.todos || []) {
    if (waiting.has(change.id)) continue;
    const { note, deleted, ...todo } = change;
    const at = replica.todos.findIndex((entry) => entry.todo.id === todo.id);
    if (deleted) {
      if (at >= 0) replica.todos.splice(at, 1);
    } else if (at >= 0) {
      replica.todos[at] = { note, todo };
    } else {
      replica.todos.push({ note, todo });
    }
  }
  replica.cursor = changes.cursor;
}

// ---- Server ---------------------------------------------------------------

/** A failed call: `refused` if sending it again won't help. */
class ApiError extends Error {
  constructor(status, message) {
    super(`${status}: ${message}`);
    this.status = status;
    this.refused = status >= 400 && status < 500 && ![401, 403, 408, 429].includes(status);
  }
}

async function call(method, path, body) {
  const headers = { Authorization: `Bearer ${localStorage.getItem(TOKEN_KEY) || ""}` };
  if (body !== undefined) headers["Content-Type"] = "application/json";
  const response = await fetch(path, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
    cache: "no-store",
  });
  if (!response.ok) {
    let message = response.statusText;
    try {
      message = (await response.json()).error || message;
    } catch {
      // Not JSON: keep the status text.
    }
    throw new ApiError(response.status, message);
  }
  return response.status === 204 ? null : response.json();
}

async function send(op) {
  const id = op.id && encodeURIComponent(op.id);
  if (op.kind === "add") return call("POST", "/api/todos", op.body);
  if (op.kind === "update") return call("PATCH", `/api/todos/${id}`, op.patch);
  try {
    return await call("DELETE", `/api/todos/${id}`);
  } catch (error) {
    if (error.status === 404) return null; // already gone
    throw error;
  }
}

function describe(op) {
  if (op.kind === "add") return `add "${op.body.text}"`;
  const entry = find(op.id);
  const what = entry ? `"${entry.todo.text}"` : op.id;
  return `${op.kind === "update" ? "edit" : "delete"} ${what}`;
}

const sync = { running: null, again: false, problem: null, refused: null };

/** Runs a sync, or one more after the running one. */
function syncNow() {
  if (sync.running) {
    sync.again = true;
    return sync.running;
  }
  sync.running = (async () => {
    do {
      sync.again = false;
      await syncOnce();
    } while (sync.again);
    sync.running = null;
    render();
  })();
  render();
  return sync.running;
}

async function syncOnce() {
  if (!localStorage.getItem(TOKEN_KEY)) {
    sync.problem = "no token";
    return;
  }
  sync.problem = null;
  try {
    while (replica.outbox.length > 0) {
      const pending = replica.outbox[0];
      try {
        await send(pending.op);
      } catch (error) {
        if (!(error instanceof ApiError && error.refused)) throw error;
        sync.refused = `${describe(pending.op)}: ${error.message}`;
        replica.resync = true;
      }
      replica.outbox = replica.outbox.filter((p) => p.seq !== pending.seq);
      save();
    }
    let since = replica.resync ? 0 : replica.cursor;
    let changes = await call("GET", `/api/sync?since=${since}`);
    // A cursor going back means a new database: start over.
    if (changes.cursor < since) {
      since = 0;
      changes = await call("GET", "/api/sync?since=0");
    }
    merge(changes, since);
    save();
  } catch (error) {
    console.warn("sync failed", error);
    if (error instanceof ApiError) {
      sync.problem = error.status === 401 ? "token refused" : error.message;
    } else {
      sync.problem = "offline";
    }
  }
}

/** After a local change: save, redraw, send. */
function changed() {
  save();
  render();
  syncNow();
}

// ---- Views ----------------------------------------------------------------

/** Tiny element builder: never sets HTML, so todo text can't inject any. */
function h(tag, props = {}, ...children) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined || value === null || value === false) continue;
    if (key.startsWith("on")) el.addEventListener(key.slice(2), value);
    else if (key === "class") el.className = value;
    else if (key in el && typeof value !== "string") el[key] = value;
    else el.setAttribute(key, value === true ? "" : value);
  }
  for (const child of children.flat(Infinity)) {
    if (child !== null && child !== undefined && child !== false) el.append(child);
  }
  return el;
}

const closed = (todo) => todo.status === "done" || todo.status === "cancelled";
const PRIORITY = ["highest", "high", "medium", "normal", "low", "lowest"];

/** Pinned first, then by due date (none last), then priority. */
function byUrgency(a, b) {
  return (
    Number(Boolean(b.pinned)) - Number(Boolean(a.pinned)) ||
    (a.due || "9999").localeCompare(b.due || "9999") ||
    PRIORITY.indexOf(a.priority) - PRIORITY.indexOf(b.priority)
  );
}

function dueClass(todo) {
  if (!todo.due || closed(todo)) return "";
  const today = isoDate();
  if (todo.due < today) return "late";
  if (todo.due === today) return "today";
  if (todo.due <= endOfWeek()) return "week";
  return "";
}

function todoRow(entry, showNote) {
  const { todo } = entry;
  const meta = h(
    "div",
    { class: "meta" },
    todo.due && h("span", { class: dueClass(todo) }, todo.due),
    todo.priority && todo.priority !== "normal" && h("span", { class: `prio-${todo.priority}` }, todo.priority),
    todo.pinned && h("span", {}, "📌"),
    todo.recur && h("span", {}, `🔁 ${todo.recur.unit}`),
    (todo.tags || []).map((tag) => h("span", {}, `#${tag}`)),
    showNote && h("span", {}, entry.note),
  );
  return h(
    "li",
    { class: `todo ${todo.status}${closed(todo) ? " closed" : ""}` },
    h("input", {
      type: "checkbox",
      checked: closed(todo),
      "aria-label": "Done",
      onchange: (event) => updateTodo(todo.id, { status: event.target.checked ? "done" : "todo" }),
    }),
    h("div", { class: "body", onclick: () => openForm(entry) }, h("div", { class: "text" }, todo.text), meta),
  );
}

function section(title, entries, showNote) {
  if (entries.length === 0) return null;
  return [h("h2", {}, `${title} (${entries.length})`), h("ul", {}, entries.map((e) => todoRow(e, showNote)))];
}

function dueView() {
  const today = isoDate();
  const week = endOfWeek();
  const open = replica.todos.filter((e) => !closed(e.todo)).sort((a, b) => byUrgency(a.todo, b.todo));
  const late = open.filter((e) => e.todo.due && e.todo.due < today);
  const now = open.filter((e) => e.todo.due === today);
  const soon = open.filter((e) => e.todo.due && e.todo.due > today && e.todo.due <= week);
  const pinned = open.filter((e) => e.todo.pinned && !(e.todo.due && e.todo.due <= week));
  const parts = [
    section("Late", late, true),
    section("Today", now, true),
    section("This week", soon, true),
    section("Pinned", pinned, true),
  ].filter(Boolean);
  if (parts.length === 0) {
    return h("p", { class: "empty" }, replica.todos.length ? "Nothing due this week." : "No todos yet.");
  }
  return parts;
}

function notesView() {
  if (replica.notes.length === 0) return h("p", { class: "empty" }, "No notes yet.");
  return h(
    "ul",
    {},
    replica.notes.map((note) => {
      const todos = replica.todos.filter((e) => e.note === note);
      const open = todos.filter((e) => !closed(e.todo)).length;
      return h(
        "li",
        { class: "note" },
        h("a", { href: `#/note/${encodeURIComponent(note)}` }, h("span", {}, note), h("span", { class: "count" }, `${open} open`)),
      );
    }),
  );
}

function noteView(note) {
  const todos = replica.todos.filter((e) => e.note === note);
  const open = todos.filter((e) => !closed(e.todo)).sort((a, b) => byUrgency(a.todo, b.todo));
  const done = todos.filter((e) => closed(e.todo));
  return [
    h("h1", {}, note),
    open.length ? h("ul", {}, open.map((e) => todoRow(e, false))) : h("p", { class: "empty" }, "Nothing open."),
    done.length > 0 &&
      h("details", {}, h("summary", {}, `Done (${done.length})`), h("ul", {}, done.map((e) => todoRow(e, false)))),
  ];
}

/** The page the URL hash names. */
function route() {
  const hash = location.hash.replace(/^#\/?/, "");
  if (hash.startsWith("note/")) return { page: "note", note: decodeURIComponent(hash.slice(5)) };
  if (hash === "notes") return { page: "notes" };
  return { page: "due" };
}

function render() {
  const { page, note } = route();
  const main = document.getElementById("main");
  const view = page === "note" ? noteView(note) : page === "notes" ? notesView() : dueView();
  main.replaceChildren(...[view].flat(Infinity).filter(Boolean));
  for (const link of document.querySelectorAll("nav a")) {
    link.classList.toggle("active", link.dataset.page === page || (page === "note" && link.dataset.page === "notes"));
  }
  const status = document.getElementById("status");
  const parts = [];
  if (sync.problem) parts.push(sync.problem);
  if (replica.outbox.length) parts.push(`${replica.outbox.length} to send`);
  status.textContent = sync.refused ? `refused: ${sync.refused}` : parts.join(" · ");
  status.classList.toggle("error", Boolean(sync.refused) || sync.problem === "token refused");
  document.getElementById("sync").classList.toggle("spin", Boolean(sync.running));
}

// ---- Form -----------------------------------------------------------------

let editing = null;

function openForm(entry) {
  editing = entry ? entry.todo.id : null;
  const form = document.getElementById("form");
  const todo = entry ? entry.todo : {};
  const { note } = route();
  document.getElementById("form-title").textContent = entry ? "Edit" : "Add";
  form.text.value = todo.text || "";
  form.note.value = entry ? entry.note : note || replica.notes[0] || "";
  form.note.disabled = Boolean(entry); // moving between notes isn't an edit
  form.due.value = todo.due || "";
  form.priority.value = todo.priority || "normal";
  form.recur.value = todo.recur ? todo.recur.unit : "";
  form.status.value = todo.status || "todo";
  form.tags.value = (todo.tags || []).join(", ");
  form.pinned.checked = Boolean(todo.pinned);
  document.getElementById("delete").hidden = !entry;
  document.getElementById("note-list").replaceChildren(...replica.notes.map((n) => h("option", { value: n })));
  document.getElementById("form-dialog").showModal();
  if (!entry) form.text.focus();
}

function formFields(form) {
  const tags = form.tags.value
    .split(/[\s,]+/)
    .map((tag) => tag.replace(/^#/, ""))
    .filter(Boolean);
  return {
    text: form.text.value.trim(),
    note: form.note.value.trim().replace(/\.md$/, ""),
    due: form.due.value || null,
    priority: form.priority.value,
    recur: form.recur.value ? { every: 1, unit: form.recur.value } : null,
    tags,
    pinned: form.pinned.checked,
  };
}

/** Only what changed, so an edit doesn't undo one made elsewhere. */
function diff(todo, fields, status) {
  const patch = {};
  if (fields.text !== todo.text) patch.text = fields.text;
  if (fields.due !== (todo.due || null)) patch.due = fields.due;
  if (fields.priority !== (todo.priority || "normal")) patch.priority = fields.priority;
  if (JSON.stringify(fields.recur) !== JSON.stringify(todo.recur || null)) patch.recur = fields.recur;
  if (fields.tags.join(" ") !== (todo.tags || []).join(" ")) patch.tags = fields.tags;
  if (fields.pinned !== Boolean(todo.pinned)) patch.pinned = fields.pinned;
  if (status !== todo.status) patch.status = status;
  return patch;
}

function setupForm() {
  const dialog = document.getElementById("form-dialog");
  const form = document.getElementById("form");
  dialog.addEventListener("close", () => {
    if (dialog.returnValue !== "save") return;
    const fields = formFields(form);
    if (!fields.text || (!editing && !fields.note)) return;
    if (editing) {
      const entry = find(editing);
      if (!entry) return;
      const patch = diff(entry.todo, fields, form.status.value);
      if (Object.keys(patch).length) updateTodo(editing, patch);
    } else {
      addTodo(fields, form.status.value);
    }
  });
  document.getElementById("delete").addEventListener("click", () => {
    if (editing && confirm("Delete this todo?")) {
      deleteTodo(editing);
      dialog.close("deleted");
    }
  });
  document.getElementById("add").addEventListener("click", () => openForm(null));
}

function setupToken() {
  const dialog = document.getElementById("token-dialog");
  const form = document.getElementById("token-form");
  const open = () => {
    form.token.value = localStorage.getItem(TOKEN_KEY) || "";
    dialog.showModal();
  };
  dialog.addEventListener("close", () => {
    if (dialog.returnValue !== "save") return;
    localStorage.setItem(TOKEN_KEY, form.token.value.trim());
    syncNow();
  });
  document.getElementById("settings").addEventListener("click", open);
  document.getElementById("reset").addEventListener("click", () => {
    const pending = replica.outbox.length;
    const warning = pending ? ` ${pending} change(s) not sent yet will be lost.` : "";
    if (!confirm(`Forget the local copy and fetch everything again?${warning}`)) return;
    replica = emptyReplica();
    sync.refused = null;
    save();
    dialog.close("reset");
    syncNow();
  });
  if (!localStorage.getItem(TOKEN_KEY)) open();
}

// ---- Start ----------------------------------------------------------------

window.addEventListener("hashchange", render);
window.addEventListener("online", () => syncNow());
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible") syncNow();
});
document.getElementById("sync").addEventListener("click", () => {
  sync.refused = null;
  syncNow();
});
setInterval(() => {
  if (document.visibilityState === "visible") syncNow();
}, AUTO_SYNC_MS);

setupForm();
setupToken();
render();
syncNow();
if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("sw.js").catch((error) => console.warn("no offline shell", error));
}
