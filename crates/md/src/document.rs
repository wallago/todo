//! A whole note: tasks plus everything around them, kept byte for byte.

use std::fmt;

use todo_core::{Id, Todo};

use crate::{
    Error,
    line::{parse_line, write_line},
};

/// A parsed markdown note.
///
/// Lines that aren't tasks, and tasks that were never changed, are written
/// back exactly as they were read. Only a task you add or update is
/// rewritten, so saving never reformats the rest of the note.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    /// Every line of the note, without its `\n`.
    lines: Vec<Line>,
}

/// One line of the note.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
    /// Anything that is not a task, as read (including any `\r`).
    Raw(String),
    /// A task line.
    Task(Task),
}

/// A task line and what is needed to write it back.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Task {
    /// The parsed todo.
    todo: Todo,
    /// Indent and bullet, e.g. `"  - "`.
    prefix: String,
    /// The line as read, until the todo changes.
    original: Option<String>,
    /// The line ended in `\r\n`.
    crlf: bool,
}

impl Document {
    /// Parses a note. Never fails: anything that isn't a task is kept as text.
    ///
    /// Frontmatter and fenced code blocks are never read as tasks.
    #[must_use]
    pub fn parse(src: &str) -> Self {
        let mut lines = Vec::new();
        let mut in_frontmatter = src.starts_with("---\n") || src.starts_with("---\r\n");
        let mut in_code = false;
        for (n, raw) in src.split('\n').enumerate() {
            let (text, crlf) = match raw.strip_suffix('\r') {
                Some(text) => (text, true),
                None => (raw, false),
            };
            let fence = text.trim_start();
            if in_frontmatter {
                in_frontmatter = n == 0 || !matches!(text, "---" | "...");
            } else if fence.starts_with("```") || fence.starts_with("~~~") {
                in_code = !in_code;
            } else if !in_code && let Some((prefix, todo)) = parse_line(text) {
                lines.push(Line::Task(Task {
                    todo,
                    prefix: prefix.to_owned(),
                    original: Some(text.to_owned()),
                    crlf,
                }));
                continue;
            }
            lines.push(Line::Raw(raw.to_owned()));
        }
        Self { lines }
    }

    /// Every todo, in the order they appear in the note.
    pub fn todos(&self) -> impl Iterator<Item = &Todo> {
        self.tasks().map(|task| &task.todo)
    }

    /// The todo with this id.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&Todo> {
        self.todos().find(|todo| &todo.id == id)
    }

    /// Adds a todo after the last task in the note, or at the end if there
    /// are none.
    ///
    /// # Errors
    ///
    /// [`Error::MissingId`] if the todo has no id, [`Error::DuplicateId`] if
    /// another todo already uses it.
    pub fn add(&mut self, todo: Todo) -> Result<(), Error> {
        if todo.id.0.is_empty() {
            return Err(Error::MissingId);
        }
        if self.get(&todo.id).is_some() {
            return Err(Error::DuplicateId(todo.id));
        }
        let at = match self.lines.iter().rposition(|l| matches!(l, Line::Task(_))) {
            Some(last) => last + 1,
            // Before the empty piece after a final newline, so it stays last.
            None if self
                .lines
                .last()
                .is_some_and(|l| *l == Line::Raw(String::new())) =>
            {
                self.lines.len() - 1
            }
            None => self.lines.len(),
        };
        let crlf = self.lines.iter().any(|l| match l {
            Line::Raw(raw) => raw.ends_with('\r'),
            Line::Task(task) => task.crlf,
        });
        self.lines.insert(
            at,
            Line::Task(Task {
                todo,
                prefix: "- ".to_owned(),
                original: None,
                crlf,
            }),
        );
        Ok(())
    }

    /// Changes the todo with this id; only its line is rewritten.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no todo has this id. [`Error::DuplicateId`] or
    /// [`Error::MissingId`] if the change gives it an id that is taken or
    /// empty; the todo is then left as it was.
    pub fn update(&mut self, id: &Id, change: impl FnOnce(&mut Todo)) -> Result<&Todo, Error> {
        let index = self
            .index_of(id)
            .ok_or_else(|| Error::NotFound(id.clone()))?;
        let mut todo = self.task(index).todo.clone();
        change(&mut todo);
        if todo.id != *id {
            if todo.id.0.is_empty() {
                return Err(Error::MissingId);
            }
            if self.get(&todo.id).is_some() {
                return Err(Error::DuplicateId(todo.id));
            }
        }
        let task = self.task_mut(index);
        if task.todo != todo {
            task.todo = todo;
            task.original = None;
        }
        Ok(&task.todo)
    }

    /// Removes the todo with this id and its line.
    ///
    /// # Errors
    ///
    /// [`Error::NotFound`] if no todo has this id.
    pub fn remove(&mut self, id: &Id) -> Result<Todo, Error> {
        let index = self
            .index_of(id)
            .ok_or_else(|| Error::NotFound(id.clone()))?;
        match self.lines.remove(index) {
            Line::Task(task) => Ok(task.todo),
            Line::Raw(_) => Err(Error::NotFound(id.clone())),
        }
    }

    /// Gives an id to every task that has none, or whose id an earlier task
    /// already uses. `new_id` must return ids not used in the note.
    /// Returns how many tasks changed.
    ///
    /// Call it once after loading a note so every todo can be addressed.
    pub fn ensure_ids(&mut self, mut new_id: impl FnMut() -> Id) -> usize {
        let mut seen = std::collections::HashSet::new();
        let mut changed = 0;
        for line in &mut self.lines {
            if let Line::Task(task) = line
                && (task.todo.id.0.is_empty() || !seen.insert(task.todo.id.clone()))
            {
                task.todo.id = new_id();
                task.original = None;
                seen.insert(task.todo.id.clone());
                changed += 1;
            }
        }
        changed
    }

    /// Every task line.
    fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.lines.iter().filter_map(|line| match line {
            Line::Task(task) => Some(task),
            Line::Raw(_) => None,
        })
    }

    /// Line index of the todo with this id.
    fn index_of(&self, id: &Id) -> Option<usize> {
        self.lines
            .iter()
            .position(|line| matches!(line, Line::Task(task) if &task.todo.id == id))
    }

    /// The task at a line index known to hold one.
    fn task(&self, index: usize) -> &Task {
        match &self.lines[index] {
            Line::Task(task) => task,
            Line::Raw(_) => unreachable!("index_of only returns task lines"),
        }
    }

    /// Mutable version of [`Self::task`].
    fn task_mut(&mut self, index: usize) -> &mut Task {
        match &mut self.lines[index] {
            Line::Task(task) => task,
            Line::Raw(_) => unreachable!("index_of only returns task lines"),
        }
    }
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (n, line) in self.lines.iter().enumerate() {
            if n > 0 {
                f.write_str("\n")?;
            }
            match line {
                Line::Raw(raw) => f.write_str(raw)?,
                Line::Task(task) => {
                    match &task.original {
                        Some(original) => f.write_str(original)?,
                        None => f.write_str(&write_line(&task.prefix, &task.todo))?,
                    }
                    if task.crlf {
                        f.write_str("\r")?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use todo_core::{Priority, Status};

    use super::*;

    /// The sample vault note.
    const SAMPLE: &str = include_str!("../../../vault/todo.md");

    /// Shorthand for ids in tests.
    fn id(s: &str) -> Id {
        Id(s.to_owned())
    }

    /// Shorthand for dates in tests.
    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default()
    }

    #[test]
    fn sample_is_written_back_unchanged() {
        assert_eq!(Document::parse(SAMPLE).to_string(), SAMPLE);
    }

    #[test]
    fn sample_has_every_task_and_nothing_else() {
        let doc = Document::parse(SAMPLE);
        let ids: Vec<&str> = doc.todos().map(|t| t.id.0.as_str()).collect();
        assert_eq!(
            ids,
            [
                "a1b2c3", "d4e5f6", "g7h8i9", "j0k1l2", "m3n4o5", "p6q7r8", "s9t0u1", "v2w3x4",
                "y5z6a7", "b8c9d0", "e1f2g3", "h4i5j6", "", "k7l8m9", "n0o1p2", "q3r4s5", "t6u7v8",
                "w9x0y1", "z2a3b4", "c5d6e7", "f8g9h0", "i1j2k3", "l4m5n6", "o7p8q9",
            ]
        );
    }

    #[test]
    fn sample_edge_cases() -> Result<(), String> {
        let doc = Document::parse(SAMPLE);
        let get = |s: &str| doc.get(&id(s)).ok_or(format!("missing {s}"));

        let reordered = get("t6u7v8")?;
        assert_eq!(reordered.text, "Fields in a different order");
        assert_eq!(reordered.priority, Priority::High);
        assert_eq!(reordered.due, Some(date(2026, 11, 2)));
        assert_eq!(reordered.created, Some(date(2026, 10, 23)));
        assert_eq!(reordered.tags, ["work"]);

        assert_eq!(
            get("n0o1p2")?.text,
            "Text that mentions `#not-a-tag` in code and an issue number #42"
        );
        assert_eq!(get("q3r4s5")?.text, "Emoji in the text 🎉 is not a field");
        assert_eq!(get("o7p8q9")?.text, "Extra spaces   around   the text");
        assert_eq!(get("e1f2g3")?.status, Status::Done);
        assert_eq!(get("h4i5j6")?.status, Status::Cancelled);
        assert_eq!(get("w9x0y1")?.status, Status::Todo);
        assert_eq!(get("y5z6a7")?.tags, ["dev/hardware"]);
        Ok(())
    }

    #[test]
    fn update_rewrites_only_that_line() -> Result<(), Error> {
        let mut doc = Document::parse(SAMPLE);
        doc.update(&id("d4e5f6"), |t| t.complete(date(2026, 10, 24)))?;
        let out = doc.to_string();
        let changed: Vec<(&str, &str)> = SAMPLE
            .lines()
            .zip(out.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(
            changed,
            [(
                "- [ ] Buy milk 📅 2026-10-24 #errand ^d4e5f6",
                "- [x] Buy milk 📅 2026-10-24 ✅ 2026-10-24 #errand ^d4e5f6"
            )]
        );
        Ok(())
    }

    #[test]
    fn update_keeps_indent_and_bullet() -> Result<(), Error> {
        let mut doc = Document::parse("- [ ] a ^p\n    * [ ] b ^c\n");
        doc.update(&id("c"), |t| t.priority = Priority::Low)?;
        assert_eq!(doc.to_string(), "- [ ] a ^p\n    * [ ] b 🔽 ^c\n");
        Ok(())
    }

    #[test]
    fn update_refuses_a_taken_id() {
        let mut doc = Document::parse("- [ ] a ^a\n- [ ] b ^b\n");
        assert_eq!(
            doc.update(&id("b"), |t| t.id = id("a")),
            Err(Error::DuplicateId(id("a")))
        );
        assert_eq!(doc.update(&id("x"), |_| ()), Err(Error::NotFound(id("x"))));
        assert_eq!(doc.to_string(), "- [ ] a ^a\n- [ ] b ^b\n");
    }

    #[test]
    fn add_goes_after_the_last_task() -> Result<(), Error> {
        let mut doc = Document::parse("# Todo\n\n- [ ] a ^a\n\nNotes\n");
        doc.add(Todo::new(id("b"), "b", date(2026, 10, 24)))?;
        assert_eq!(
            doc.to_string(),
            "# Todo\n\n- [ ] a ^a\n- [ ] b ➕ 2026-10-24 ^b\n\nNotes\n"
        );
        assert_eq!(
            doc.add(Todo::new(id("a"), "again", date(2026, 10, 24))),
            Err(Error::DuplicateId(id("a")))
        );
        assert_eq!(
            doc.add(Todo::new(id(""), "no id", date(2026, 10, 24))),
            Err(Error::MissingId)
        );
        Ok(())
    }

    #[test]
    fn add_to_a_note_without_tasks() -> Result<(), Error> {
        for (src, expected) in [
            ("", "- [ ] b ^b\n"),
            ("# Todo\n", "# Todo\n- [ ] b ^b\n"),
            ("# Todo", "# Todo\n- [ ] b ^b"),
            ("# Todo\r\n", "# Todo\r\n- [ ] b ^b\r\n"),
        ] {
            let mut doc = Document::parse(src);
            let mut todo = Todo::new(id("b"), "b", date(2026, 10, 24));
            todo.created = None;
            doc.add(todo)?;
            assert_eq!(doc.to_string(), expected, "{src:?}");
        }
        Ok(())
    }

    #[test]
    fn remove_drops_the_line() -> Result<(), Error> {
        let mut doc = Document::parse("- [ ] a ^a\n- [ ] b ^b\n");
        assert_eq!(doc.remove(&id("a"))?.text, "a");
        assert_eq!(doc.to_string(), "- [ ] b ^b\n");
        Ok(())
    }

    #[test]
    fn ensure_ids_fills_missing_and_duplicates() {
        let mut doc = Document::parse("- [ ] a\n- [ ] b ^b\n- [ ] copy ^b\n");
        let mut next = 0;
        let changed = doc.ensure_ids(|| {
            next += 1;
            Id(format!("new{next}"))
        });
        assert_eq!(changed, 2);
        assert_eq!(
            doc.to_string(),
            "- [ ] a ^new1\n- [ ] b ^b\n- [ ] copy ^new2\n"
        );
    }

    #[test]
    fn code_blocks_and_frontmatter_are_not_tasks() {
        let src = "---\ntags: [todo]\n- [ ] in frontmatter\n---\n```\n- [ ] in code\n```\n- [ ] real ^r\n";
        let doc = Document::parse(src);
        let texts: Vec<&str> = doc.todos().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["real"]);
    }

    proptest! {
        #[test]
        fn any_text_is_written_back_unchanged(src in "(- \\[[ x/-]\\] [a-z #^0-9]{0,12}|```|---|[^\n]{0,20}|\r)?(\n(- \\[[ x/-]\\] [a-z #^0-9]{0,12}|```|[^\n]{0,20}|\r))*") {
            prop_assert_eq!(Document::parse(&src).to_string(), src);
        }
    }
}
