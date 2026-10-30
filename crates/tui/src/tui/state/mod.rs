//! What the UI shows, and how keys change it.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use todo_client::{Id, NewTodo, Status, TodoPatch};

use crate::config::Keys;

/// The `y/n` popup before a delete.
mod confirm;
/// The add and edit form.
mod form;
/// The manager page: one note's todos.
mod manager;
/// The note finder popup.
mod picker;

pub(super) use confirm::Confirm;
pub(super) use form::{Field, Form, PRIORITIES, STATUSES};
pub(super) use manager::Manager;
pub(super) use picker::Picker;

/// What a key did to a popup.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    /// Still open.
    Open,
    /// Closed without doing anything.
    Cancel,
    /// Closed with an answer.
    Submit,
}

/// Which side owns the keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Focus {
    /// Page navigation on the left.
    #[default]
    Sidebar,
    /// The current page.
    Page,
}

/// Page shown on the right, picked in the sidebar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Page {
    /// One note's todos, the note picked with `/`.
    #[default]
    Manager,
    /// How much of each note is done.
    Stats,
    /// Name, version and description of the crate.
    About,
}

impl Page {
    /// Every page, in sidebar order.
    pub(super) const ALL: [Self; 3] = [Self::Manager, Self::Stats, Self::About];

    /// Label in the sidebar.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Manager => "manager",
            Self::Stats => "stats",
            Self::About => "about",
        }
    }

    /// Position in [`Self::ALL`].
    pub(super) fn index(self) -> usize {
        Self::ALL.iter().position(|&page| page == self).unwrap_or(0)
    }

    /// The page after (or before) this one, or `self` past either end.
    fn step(self, forward: bool) -> Self {
        let at = self.index();
        let to = if forward {
            at.saturating_add(1)
        } else {
            at.saturating_sub(1)
        };
        Self::ALL.get(to).copied().unwrap_or(self)
    }
}

/// Server calls a key asks for; the event loop runs them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Action {
    /// Reload the notes, then what the current page shows.
    Refresh,
    /// Reload the manager's note.
    LoadTodos,
    /// Reload every note's counts.
    LoadStats,
    /// Tick the selected todo, or reopen it if it's closed.
    Toggle,
    /// Add this todo to the manager's note, then give it this status.
    Add(NewTodo, Status),
    /// Apply this patch to this todo.
    Change(Id, TodoPatch),
    /// Delete this todo.
    Delete(Id),
}

/// Done and total todos of one note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NoteStats {
    /// Note path, like `projects/todo.md`.
    pub(super) note: String,
    /// Done or cancelled todos.
    pub(super) done: usize,
    /// Every todo.
    pub(super) total: usize,
}

/// Application state.
#[derive(Debug)]
pub(super) struct State {
    /// Cleared to leave the event loop.
    pub(super) running: bool,
    /// Which side owns the keyboard.
    pub(super) focus: Focus,
    /// Page shown and selected in the sidebar.
    pub(super) page: Page,
    /// Note paths from the server, like `projects/todo.md`.
    pub(super) notes: Vec<String>,
    /// The manager page.
    pub(super) manager: Manager,
    /// The note finder, while open.
    pub(super) picker: Option<Picker>,
    /// The add or edit form, while open.
    pub(super) form: Option<Form>,
    /// The delete confirmation, while open.
    pub(super) confirm: Option<Confirm>,
    /// The stats page, one row per note.
    pub(super) stats: Vec<NoteStats>,
    /// Last server error, shown until the next successful call.
    pub(super) error: Option<String>,
    /// Movement keys from the config.
    pub(super) keys: Keys,
}

impl Default for State {
    fn default() -> Self {
        Self {
            running: true,
            focus: Focus::default(),
            page: Page::default(),
            notes: Vec::new(),
            manager: Manager::default(),
            picker: None,
            form: None,
            confirm: None,
            stats: Vec::new(),
            error: None,
            keys: Keys::default(),
        }
    }
}

impl State {
    /// Replaces the notes. The manager keeps its note, even a new one with
    /// no file yet; with none open, it opens the first one.
    pub(super) fn set_notes(&mut self, notes: Vec<String>) {
        if self.manager.note.is_none() {
            self.manager.open(notes.first().cloned());
        }
        self.notes = notes;
    }

    /// Applies a key press, returning the server call it needs, if any.
    pub(super) fn on_key(&mut self, key: KeyEvent) -> Option<Action> {
        let ctrl_c =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
        if ctrl_c {
            // Raw mode turns Ctrl-C into a key, so it never reaches the signal handler.
            self.running = false;
            return None;
        }
        // The popup takes every key, `q` included, while open.
        if let Some(picker) = &mut self.picker {
            return match key.code {
                KeyCode::Esc => {
                    self.picker = None;
                    None
                }
                KeyCode::Enter => {
                    let note = picker.choice();
                    self.picker = None;
                    note.map(|note| {
                        // A new note has no file until its first todo: ask for it.
                        if !self.notes.contains(&note) {
                            self.form = Some(Form::new(today()));
                        }
                        self.manager.open(Some(note));
                        Action::LoadTodos
                    })
                }
                _ => {
                    picker.on_key(key, self.keys);
                    None
                }
            };
        }
        // Popups take every key while open.
        if let Some(form) = &mut self.form {
            return match form.on_key(key, self.keys) {
                Outcome::Open => None,
                Outcome::Cancel => {
                    self.form = None;
                    None
                }
                Outcome::Submit => self.form.take().and_then(Form::action),
            };
        }
        if self.confirm.is_some() {
            return match Confirm::on_key(key.code) {
                Outcome::Open => None,
                Outcome::Cancel => {
                    self.confirm = None;
                    None
                }
                Outcome::Submit => self
                    .confirm
                    .take()
                    .map(|confirm| Action::Delete(confirm.id)),
            };
        }
        let keys = self.keys;
        let is =
            |arrow: KeyCode, letter: char| key.code == arrow || key.code == KeyCode::Char(letter);
        let (up, down) = (is(KeyCode::Up, keys.up), is(KeyCode::Down, keys.down));
        let (left, right) = (is(KeyCode::Left, keys.left), is(KeyCode::Right, keys.right));
        match (self.focus, self.page, key.code) {
            (_, _, KeyCode::Char('q')) => self.running = false,
            (_, _, KeyCode::Char('r')) => return Some(Action::Refresh),
            (_, Page::Manager, KeyCode::Char('/')) => {
                self.focus = Focus::Page;
                self.picker = Some(Picker::new(self.notes.clone()));
            }
            (Focus::Sidebar, _, _) if up || down => {
                let before = self.page;
                self.page = self.page.step(down);
                if self.page == Page::Stats && before != Page::Stats {
                    return Some(Action::LoadStats);
                }
            }
            (Focus::Sidebar, _, KeyCode::Tab | KeyCode::Enter) => self.focus = Focus::Page,
            (Focus::Sidebar, _, _) if right => self.focus = Focus::Page,
            (Focus::Page, _, KeyCode::Tab | KeyCode::Esc) => self.focus = Focus::Sidebar,
            (Focus::Page, _, _) if left => self.focus = Focus::Sidebar,
            (Focus::Page, Page::Manager, _) if up || down => self.manager.step(down),
            (Focus::Page, Page::Manager, KeyCode::Enter) if self.manager.selected().is_some() => {
                return Some(Action::Toggle);
            }
            (_, Page::Manager, KeyCode::Char('a')) if self.manager.note.is_some() => {
                self.form = Some(Form::new(today()));
            }
            (_, Page::Manager, KeyCode::Char('c')) => {
                self.form = self
                    .manager
                    .selected()
                    .map(|todo| Form::edit(todo, today()));
            }
            (_, Page::Manager, KeyCode::Char('d')) => {
                self.confirm = self.manager.selected().map(|todo| Confirm {
                    id: todo.id.clone(),
                    text: todo.text.clone(),
                });
            }
            _ => {}
        }
        None
    }

    /// Keys that do something right now, as `(keys, what they do)`.
    pub(super) fn key_bindings(&self) -> &'static [(&'static str, &'static str)] {
        if let Some(form) = &self.form {
            return match form.field {
                Field::Title => &[("tab", "next"), ("enter", "save"), ("esc", "cancel")],
                Field::Due => &[
                    ("arrows", "day"),
                    ("pgup/dn", "month"),
                    ("t", "today"),
                    ("⌫", "none"),
                    ("tab", "next"),
                    ("enter", "save"),
                ],
                _ => &[
                    ("↑↓", "choose"),
                    ("tab", "next"),
                    ("enter", "save"),
                    ("esc", "cancel"),
                ],
            };
        }
        if self.confirm.is_some() {
            return &[("y", "delete"), ("n", "keep")];
        }
        if self.picker.is_some() {
            return &[
                ("type", "filter"),
                ("alt+↑↓", "choose"),
                ("enter", "open or create"),
                ("esc", "close"),
            ];
        }
        match (self.focus, self.page) {
            (Focus::Sidebar, Page::Manager) => &[
                ("↑↓", "page"),
                ("tab", "enter"),
                ("/", "find note"),
                ("q", "quit"),
            ],
            (Focus::Sidebar, _) => &[
                ("↑↓", "page"),
                ("tab", "enter"),
                ("r", "refresh"),
                ("q", "quit"),
            ],
            (Focus::Page, Page::Manager) => &[
                ("/", "note"),
                ("↑↓", "move"),
                ("enter", "done"),
                ("a/c/d", "add/edit/del"),
                ("tab", "sidebar"),
                ("q", "quit"),
            ],
            (Focus::Page, _) => &[("tab", "sidebar"), ("r", "refresh"), ("q", "quit")],
        }
    }
}

/// Today, in local time: the calendar's starting point.
fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use todo_client::{Id, Todo};

    use super::*;

    /// A press of `code` with no modifier.
    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Presses each character of `text`.
    fn type_text(state: &mut State, text: &str) {
        for c in text.chars() {
            state.on_key(press(KeyCode::Char(c)));
        }
    }

    /// A state with three notes, the first open.
    fn with_notes() -> State {
        let mut state = State::default();
        state.set_notes(vec![
            "projects/todo/main.md".into(),
            "travel/japan.md".into(),
            "todo.md".into(),
        ]);
        state
    }

    #[rstest]
    #[case::q(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))]
    #[case::ctrl_c(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))]
    fn quits(#[case] key: KeyEvent) {
        let mut state = State::default();
        state.on_key(key);
        assert!(!state.running);
    }

    #[test]
    fn opens_the_first_note_and_keeps_it_on_refresh() {
        let mut state = with_notes();
        assert_eq!(state.manager.note.as_deref(), Some("projects/todo/main.md"));
        state.manager.open(Some("new.md".into()));
        // Not on the server yet, but still the open one.
        state.set_notes(vec!["a.md".into()]);
        assert_eq!(state.manager.note.as_deref(), Some("new.md"));
    }

    /// A state on the manager page, with one todo selected.
    fn with_a_todo() -> State {
        let mut state = with_notes();
        state.focus = Focus::Page;
        state.manager.set_todos(vec![Todo::new(
            Id("a1".into()),
            "milk",
            chrono::NaiveDate::MIN,
        )]);
        state
    }

    #[test]
    fn a_opens_an_empty_form_that_adds_the_typed_text() {
        let mut state = with_a_todo();
        state.on_key(press(KeyCode::Char('a')));
        // Letters type in the title, even movement keys and `q`.
        type_text(&mut state, " jq  ");
        assert!(state.running);
        let new = NewTodo {
            text: "jq".into(),
            ..NewTodo::default()
        };
        assert_eq!(
            state.on_key(press(KeyCode::Enter)),
            Some(Action::Add(new, Status::Todo))
        );
        assert!(state.form.is_none());
    }

    #[test]
    fn esc_closes_the_form_without_saving() {
        let mut state = with_a_todo();
        state.on_key(press(KeyCode::Char('a')));
        type_text(&mut state, "x");
        assert_eq!(state.on_key(press(KeyCode::Esc)), None);
        assert!(state.form.is_none());
    }

    #[test]
    fn c_opens_the_form_on_the_selected_todo() {
        let mut state = with_a_todo();
        state.on_key(press(KeyCode::Char('c')));
        assert_eq!(state.form.as_ref().map(|f| f.title.as_str()), Some("milk"));
        type_text(&mut state, "!");
        let patch = TodoPatch {
            text: Some("milk!".into()),
            ..TodoPatch::default()
        };
        assert_eq!(
            state.on_key(press(KeyCode::Enter)),
            Some(Action::Change(Id("a1".into()), patch))
        );
    }

    #[rstest]
    #[case::yes(KeyCode::Char('y'), Some(Action::Delete(Id("a1".into()))))]
    #[case::no(KeyCode::Char('n'), None)]
    #[case::esc(KeyCode::Esc, None)]
    fn d_deletes_after_a_yes(#[case] answer: KeyCode, #[case] action: Option<Action>) {
        let mut state = with_a_todo();
        state.on_key(press(KeyCode::Char('d')));
        // Other keys wait for the answer.
        assert_eq!(state.on_key(press(KeyCode::Char('x'))), None);
        assert!(state.confirm.is_some());
        assert_eq!(state.on_key(press(answer)), action);
        assert!(state.confirm.is_none());
    }

    #[test]
    fn picking_a_new_note_opens_it_and_asks_for_a_todo() {
        let mut state = with_notes();
        state.on_key(press(KeyCode::Char('/')));
        type_text(&mut state, "zz/new");
        assert_eq!(state.on_key(press(KeyCode::Enter)), Some(Action::LoadTodos));
        assert_eq!(state.manager.note.as_deref(), Some("zz/new.md"));
        assert!(state.form.as_ref().is_some_and(|form| !form.is_edit()));
    }

    #[test]
    fn sidebar_walks_the_pages_and_loads_stats() {
        let mut state = State::default();
        assert_eq!(state.on_key(press(KeyCode::Down)), Some(Action::LoadStats));
        assert_eq!(state.page, Page::Stats);
        assert_eq!(state.on_key(press(KeyCode::Down)), None);
        assert_eq!(state.page, Page::About);
        state.on_key(press(KeyCode::Down));
        assert_eq!(state.page, Page::About);
    }

    #[test]
    fn picker_opens_the_chosen_note() {
        let mut state = with_notes();
        state.on_key(press(KeyCode::Char('/')));
        // `q` filters instead of quitting while the popup is open.
        type_text(&mut state, "jpq");
        assert!(state.running);
        state.on_key(press(KeyCode::Backspace));
        assert_eq!(
            state.picker.as_ref().and_then(Picker::selected),
            Some("travel/japan.md")
        );
        assert_eq!(state.on_key(press(KeyCode::Enter)), Some(Action::LoadTodos));
        assert!(state.picker.is_none());
        assert_eq!(state.manager.note.as_deref(), Some("travel/japan.md"));
    }

    #[test]
    fn esc_closes_the_picker_without_opening() {
        let mut state = with_notes();
        state.on_key(press(KeyCode::Char('/')));
        type_text(&mut state, "jap");
        assert_eq!(state.on_key(press(KeyCode::Esc)), None);
        assert!(state.picker.is_none());
        assert_eq!(state.manager.note.as_deref(), Some("projects/todo/main.md"));
    }

    #[test]
    fn custom_keys_move_and_enter_toggles() {
        let mut state = with_notes();
        state.keys = Keys {
            up: 'i',
            down: 'e',
            left: 'n',
            right: 'o',
        };
        state.manager.set_todos(vec![
            Todo::new(Id("a".into()), "a", chrono::NaiveDate::MIN),
            Todo::new(Id("b".into()), "b", chrono::NaiveDate::MIN),
        ]);
        state.on_key(press(KeyCode::Char('o')));
        assert_eq!(state.focus, Focus::Page);
        state.on_key(press(KeyCode::Char('e')));
        assert_eq!(state.manager.list.selected(), Some(1));
        // The old vim keys do nothing now.
        state.on_key(press(KeyCode::Char('k')));
        assert_eq!(state.manager.list.selected(), Some(1));
        state.on_key(press(KeyCode::Char('i')));
        assert_eq!(state.manager.list.selected(), Some(0));
        assert_eq!(state.on_key(press(KeyCode::Enter)), Some(Action::Toggle));
        state.on_key(press(KeyCode::Char('n')));
        assert_eq!(state.focus, Focus::Sidebar);
    }

    #[test]
    fn slash_only_works_on_the_manager() {
        let mut state = State {
            page: Page::About,
            ..State::default()
        };
        state.on_key(press(KeyCode::Char('/')));
        assert!(state.picker.is_none());
    }
}
