//! The add and edit form: title, priority, due date, status and tags.

use chrono::{Days, Months, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use todo_client::{NewTodo, Priority, Status, Todo, TodoPatch};

use super::{Action, Outcome};
use crate::config::Keys;

/// A form field, in tab order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::tui) enum Field {
    /// Free text.
    #[default]
    Title,
    /// A dropdown of [`PRIORITIES`].
    Priority,
    /// A calendar.
    Due,
    /// A dropdown of [`STATUSES`].
    Status,
    /// Checkboxes of every known tag, filtered by what's typed.
    Tags,
}

impl Field {
    /// Every field, in tab order.
    pub(in crate::tui) const ALL: [Self; 5] = [
        Self::Title,
        Self::Priority,
        Self::Due,
        Self::Status,
        Self::Tags,
    ];

    /// The next (or previous) field, wrapping around.
    fn step(self, forward: bool) -> Self {
        let len = Self::ALL.len();
        let at = Self::ALL.iter().position(|&f| f == self).unwrap_or(0);
        let to = if forward { at + 1 } else { at + len - 1 };
        Self::ALL.get(to % len).copied().unwrap_or(self)
    }
}

/// Dropdown order, highest first.
pub(in crate::tui) const PRIORITIES: [Priority; 6] = [
    Priority::Highest,
    Priority::High,
    Priority::Medium,
    Priority::Normal,
    Priority::Low,
    Priority::Lowest,
];

/// Dropdown order.
pub(in crate::tui) const STATUSES: [Status; 4] = [
    Status::Todo,
    Status::InProgress,
    Status::Done,
    Status::Cancelled,
];

/// The form's values, and the todo it edits, if any.
#[derive(Clone, Debug)]
pub(in crate::tui) struct Form {
    /// The todo being edited; `None` for a new one.
    original: Option<Todo>,
    /// The field that takes keys.
    pub(in crate::tui) field: Field,
    /// What to do.
    pub(in crate::tui) title: String,
    /// Priority.
    pub(in crate::tui) priority: Priority,
    /// Due date, if any.
    pub(in crate::tui) due: Option<NaiveDate>,
    /// Checkbox state.
    pub(in crate::tui) status: Status,
    /// Ticked tags, without the leading `#`, in the order they were ticked.
    pub(in crate::tui) tags: Vec<String>,
    /// Every tag used in the vault, sorted, plus any made here.
    pub(in crate::tui) known_tags: Vec<String>,
    /// What's typed in the tags field: filters the list, or names a new tag.
    pub(in crate::tui) tag_input: String,
    /// Highlighted row of [`Self::tag_matches`].
    pub(in crate::tui) tag_cursor: usize,
    /// Where the calendar starts when there's no due date.
    pub(in crate::tui) today: NaiveDate,
}

impl Form {
    /// An empty form for a new todo; `known_tags` are offered in the tags
    /// field.
    pub(in crate::tui) fn new(today: NaiveDate, known_tags: Vec<String>) -> Self {
        Self {
            original: None,
            field: Field::Title,
            title: String::new(),
            priority: Priority::Normal,
            due: None,
            status: Status::Todo,
            tags: Vec::new(),
            known_tags,
            tag_input: String::new(),
            tag_cursor: 0,
            today,
        }
    }

    /// A form filled from `todo`, to edit it.
    pub(in crate::tui) fn edit(todo: &Todo, today: NaiveDate, known_tags: Vec<String>) -> Self {
        Self {
            original: Some(todo.clone()),
            title: todo.text.clone(),
            priority: todo.priority,
            due: todo.due,
            status: todo.status,
            tags: todo.tags.clone(),
            ..Self::new(today, known_tags)
        }
    }

    /// Known tags containing what's typed, ignoring case.
    pub(in crate::tui) fn tag_matches(&self) -> Vec<&String> {
        let query = self.tag_input.to_lowercase();
        self.known_tags
            .iter()
            .filter(|tag| tag.to_lowercase().contains(&query))
            .collect()
    }

    /// Ticks `tag`, or unticks it if it was ticked.
    fn toggle_tag(&mut self, tag: String) {
        if let Some(at) = self.tags.iter().position(|t| *t == tag) {
            self.tags.remove(at);
        } else {
            self.tags.push(tag);
        }
    }

    /// Ticks the typed tag, making it known if it's new, and clears the input.
    fn add_typed_tag(&mut self) {
        let tag = self.tag_input.trim().trim_start_matches('#').to_owned();
        self.tag_input.clear();
        self.tag_cursor = 0;
        if tag.is_empty() {
            return;
        }
        if !self.known_tags.contains(&tag) {
            self.known_tags.push(tag.clone());
            self.known_tags.sort();
        }
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// `true` when editing an existing todo.
    pub(in crate::tui) fn is_edit(&self) -> bool {
        self.original.is_some()
    }

    /// Applies a key. `tab` moves between fields, `enter` saves (or, with
    /// a tag typed, adds that tag), `esc` cancels; the rest goes to the
    /// focused field.
    pub(in crate::tui) fn on_key(&mut self, key: KeyEvent, keys: Keys) -> Outcome {
        match key.code {
            KeyCode::Esc => return Outcome::Cancel,
            KeyCode::Enter if self.field == Field::Tags && !self.tag_input.trim().is_empty() => {
                self.add_typed_tag();
            }
            KeyCode::Enter if self.title.trim().is_empty() => return Outcome::Open,
            KeyCode::Enter => return Outcome::Submit,
            KeyCode::Tab => self.field = self.field.step(true),
            KeyCode::BackTab => self.field = self.field.step(false),
            code => self.on_field_key(code, keys),
        }
        Outcome::Open
    }

    /// A key for the focused field. Letters type in the title and tags;
    /// elsewhere the movement keys from `keys` work like arrows.
    fn on_field_key(&mut self, code: KeyCode, keys: Keys) {
        let arrow = match code {
            KeyCode::Char(c) if !matches!(self.field, Field::Title | Field::Tags) => {
                if c == keys.up {
                    KeyCode::Up
                } else if c == keys.down {
                    KeyCode::Down
                } else if c == keys.left {
                    KeyCode::Left
                } else if c == keys.right {
                    KeyCode::Right
                } else {
                    code
                }
            }
            _ => code,
        };
        match (self.field, arrow) {
            (Field::Title, KeyCode::Char(c)) => self.title.push(c),
            (Field::Title, KeyCode::Backspace) => {
                self.title.pop();
            }
            (Field::Priority, KeyCode::Up | KeyCode::Down) => {
                self.priority = pick(&PRIORITIES, self.priority, arrow == KeyCode::Down);
            }
            (Field::Status, KeyCode::Up | KeyCode::Down) => {
                self.status = pick(&STATUSES, self.status, arrow == KeyCode::Down);
            }
            (Field::Tags, KeyCode::Char(' ')) => {
                let tag = self.tag_matches().get(self.tag_cursor).map(|&t| t.clone());
                if let Some(tag) = tag {
                    self.toggle_tag(tag);
                }
            }
            (Field::Tags, KeyCode::Char(c)) => {
                self.tag_input.push(c);
                self.tag_cursor = 0;
            }
            (Field::Tags, KeyCode::Backspace) => {
                self.tag_input.pop();
                self.tag_cursor = 0;
            }
            (Field::Tags, KeyCode::Up) => self.tag_cursor = self.tag_cursor.saturating_sub(1),
            (Field::Tags, KeyCode::Down) => {
                let last = self.tag_matches().len().saturating_sub(1);
                self.tag_cursor = self.tag_cursor.saturating_add(1).min(last);
            }
            (Field::Due, KeyCode::Backspace | KeyCode::Delete) => self.due = None,
            (Field::Due, KeyCode::Char('t')) => self.due = Some(self.today),
            (Field::Due, _) => {
                let day = self.due.unwrap_or(self.today);
                let moved = match arrow {
                    KeyCode::Left => day.checked_sub_days(Days::new(1)),
                    KeyCode::Right => day.checked_add_days(Days::new(1)),
                    KeyCode::Up => day.checked_sub_days(Days::new(7)),
                    KeyCode::Down => day.checked_add_days(Days::new(7)),
                    KeyCode::PageUp => day.checked_sub_months(Months::new(1)),
                    KeyCode::PageDown => day.checked_add_months(Months::new(1)),
                    _ => return,
                };
                // The first move only opens the calendar on today.
                self.due = if self.due.is_none() {
                    Some(day)
                } else {
                    moved.or(Some(day))
                };
            }
            _ => {}
        }
    }

    /// The server call that saves the form, or `None` if an edit changed
    /// nothing.
    pub(in crate::tui) fn action(self) -> Option<Action> {
        let title = self.title.trim().to_owned();
        let Some(old) = self.original else {
            let new = NewTodo {
                text: title,
                priority: self.priority,
                due: self.due,
                tags: self.tags,
                ..NewTodo::default()
            };
            return Some(Action::Add(new, self.status));
        };
        // Only what changed, so an unchanged done todo keeps its done date.
        let patch = TodoPatch {
            text: (title != old.text).then_some(title),
            priority: (self.priority != old.priority).then_some(self.priority),
            due: (self.due != old.due).then_some(self.due),
            status: (self.status != old.status).then_some(self.status),
            tags: (self.tags != old.tags).then_some(self.tags),
        };
        (patch != TodoPatch::default()).then_some(Action::Change(old.id, patch))
    }
}

/// The option after (or before) `current` in `options`, stopping at the ends.
fn pick<T: Copy + PartialEq>(options: &[T], current: T, down: bool) -> T {
    let at = options.iter().position(|&o| o == current).unwrap_or(0);
    let to = if down {
        at.saturating_add(1)
    } else {
        at.saturating_sub(1)
    };
    options.get(to).copied().unwrap_or(current)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use ratatui::crossterm::event::KeyModifiers;
    use todo_client::Id;

    use super::*;

    /// 2026-10-30, a Friday.
    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 30).unwrap_or_default()
    }

    /// Colemak-style keys, to check letters type in the title but move elsewhere.
    fn keys() -> Keys {
        Keys {
            up: 'i',
            down: 'e',
            left: 'n',
            right: 'o',
        }
    }

    /// Presses `code` with no modifier.
    fn press(form: &mut Form, code: KeyCode) -> Outcome {
        form.on_key(KeyEvent::new(code, KeyModifiers::NONE), keys())
    }

    /// Types `text` into the focused field.
    fn type_text(form: &mut Form, text: &str) {
        for c in text.chars() {
            press(form, KeyCode::Char(c));
        }
    }

    /// Tags already used somewhere in the vault.
    fn known() -> Vec<String> {
        vec!["home".into(), "shop".into(), "work".into()]
    }

    #[test]
    fn new_todo_with_every_field() {
        let mut form = Form::new(today(), known());
        type_text(&mut form, "pine trees");
        press(&mut form, KeyCode::Tab);
        // Up the dropdown: normal -> medium -> high.
        type_text(&mut form, "ii");
        press(&mut form, KeyCode::Tab);
        // Opens on today, then a week and a day later.
        type_text(&mut form, "eeo");
        press(&mut form, KeyCode::Tab);
        type_text(&mut form, "e");
        press(&mut form, KeyCode::Tab);
        // Tick `work` by filtering, then make a new `garden` tag.
        type_text(&mut form, "wo ");
        press(&mut form, KeyCode::Backspace);
        press(&mut form, KeyCode::Backspace);
        type_text(&mut form, "#garden");
        assert_eq!(press(&mut form, KeyCode::Enter), Outcome::Open);
        assert_eq!(press(&mut form, KeyCode::Enter), Outcome::Submit);
        let new = NewTodo {
            text: "pine trees".into(),
            priority: Priority::High,
            due: NaiveDate::from_ymd_opt(2026, 11, 7),
            tags: vec!["work".into(), "garden".into()],
            ..NewTodo::default()
        };
        assert_eq!(form.action(), Some(Action::Add(new, Status::InProgress)));
    }

    #[test]
    fn enter_needs_a_title() {
        let mut form = Form::new(today(), Vec::new());
        type_text(&mut form, "  ");
        assert_eq!(press(&mut form, KeyCode::Enter), Outcome::Open);
        assert_eq!(press(&mut form, KeyCode::Esc), Outcome::Cancel);
    }

    #[test]
    fn due_can_be_cleared_or_set_to_today() {
        let mut form = Form::new(today(), Vec::new());
        form.field = Field::Due;
        press(&mut form, KeyCode::PageDown);
        press(&mut form, KeyCode::PageDown);
        assert_eq!(form.due, NaiveDate::from_ymd_opt(2026, 11, 30));
        press(&mut form, KeyCode::Backspace);
        assert_eq!(form.due, None);
        press(&mut form, KeyCode::Char('t'));
        assert_eq!(form.due, Some(today()));
    }

    #[test]
    fn edit_sends_only_what_changed() {
        let mut todo = Todo::new(Id("a1".into()), "milk", today());
        todo.status = Status::Done;
        let mut form = Form::edit(&todo, today(), known());
        assert_eq!(form.clone().action(), None);
        type_text(&mut form, "!");
        form.field = Field::Priority;
        press(&mut form, KeyCode::Up);
        let patch = TodoPatch {
            text: Some("milk!".into()),
            priority: Some(Priority::Medium),
            ..TodoPatch::default()
        };
        assert_eq!(form.action(), Some(Action::Change(Id("a1".into()), patch)));
    }

    #[test]
    fn dropdowns_stop_at_the_ends_and_tab_wraps() {
        let mut form = Form::new(today(), Vec::new());
        press(&mut form, KeyCode::BackTab);
        assert_eq!(form.field, Field::Tags);
        press(&mut form, KeyCode::BackTab);
        assert_eq!(form.field, Field::Status);
        for _ in 0..9 {
            press(&mut form, KeyCode::Down);
        }
        assert_eq!(form.status, Status::Cancelled);
        press(&mut form, KeyCode::Tab);
        press(&mut form, KeyCode::Tab);
        assert_eq!(form.field, Field::Title);
    }

    #[test]
    fn tags_tick_untick_and_filter() {
        let mut form = Form::new(today(), known());
        form.field = Field::Tags;
        // Letters type here instead of moving: `e` is down in these keys.
        type_text(&mut form, "e");
        assert_eq!(form.tag_matches(), [&"home".to_owned()]);
        press(&mut form, KeyCode::Char(' '));
        assert_eq!(form.tags, ["home"]);
        press(&mut form, KeyCode::Backspace);
        assert_eq!(form.tag_matches().len(), 3);
        for code in [KeyCode::Down, KeyCode::Down, KeyCode::Down] {
            press(&mut form, code);
        }
        press(&mut form, KeyCode::Char(' '));
        press(&mut form, KeyCode::Up);
        press(&mut form, KeyCode::Char(' '));
        press(&mut form, KeyCode::Up);
        press(&mut form, KeyCode::Char(' '));
        assert_eq!(form.tags, ["work", "shop"]);
    }

    #[test]
    fn a_new_tag_is_known_and_ticked_once() {
        let mut form = Form::new(today(), known());
        form.field = Field::Tags;
        type_text(&mut form, "#car");
        press(&mut form, KeyCode::Enter);
        type_text(&mut form, "car");
        press(&mut form, KeyCode::Enter);
        assert_eq!(form.tags, ["car"]);
        assert_eq!(form.known_tags, ["car", "home", "shop", "work"]);
        assert_eq!(form.tag_input, "");
    }

    #[test]
    fn edit_sends_new_tags() {
        let mut todo = Todo::new(Id("a1".into()), "milk", today());
        todo.tags = vec!["shop".into()];
        let mut form = Form::edit(&todo, today(), known());
        assert_eq!(form.tags, ["shop"]);
        form.field = Field::Tags;
        type_text(&mut form, "home ");
        let patch = TodoPatch {
            tags: Some(vec!["shop".into(), "home".into()]),
            ..TodoPatch::default()
        };
        assert_eq!(form.action(), Some(Action::Change(Id("a1".into()), patch)));
    }
}
