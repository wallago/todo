//! A whole note.

use todo_core::Todo;

use crate::line::parse_line;

/// Every todo in a note, in order. Never fails: anything that isn't a task
/// is skipped. Frontmatter and fenced code blocks are never read as tasks.
///
/// A task without a `^id` gets an empty [`todo_core::Id`].
#[must_use]
pub fn parse_note(src: &str) -> Vec<Todo> {
    let mut todos = Vec::new();
    let mut in_frontmatter = src.starts_with("---\n") || src.starts_with("---\r\n");
    let mut in_code = false;
    for (n, raw) in src.split('\n').enumerate() {
        let text = raw.strip_suffix('\r').unwrap_or(raw);
        let fence = text.trim_start();
        if in_frontmatter {
            in_frontmatter = n == 0 || !matches!(text, "---" | "...");
        } else if fence.starts_with("```") || fence.starts_with("~~~") {
            in_code = !in_code;
        } else if !in_code && let Some((_, todo)) = parse_line(text) {
            todos.push(todo);
        }
    }
    todos
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use todo_core::{Priority, Status};

    use super::*;

    /// The sample vault note.
    const SAMPLE: &str = include_str!("../../../vault/todo.md");

    /// Shorthand for dates in tests.
    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default()
    }

    #[test]
    fn sample_has_every_task_and_nothing_else() {
        let todos = parse_note(SAMPLE);
        let ids: Vec<&str> = todos.iter().map(|t| t.id.0.as_str()).collect();
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
        let todos = parse_note(SAMPLE);
        let get = |s: &str| {
            todos
                .iter()
                .find(|t| t.id.0 == s)
                .ok_or(format!("missing {s}"))
        };

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
    fn code_blocks_and_frontmatter_are_not_tasks() {
        let src = "---\ntags: [todo]\n- [ ] in frontmatter\n---\n```\n- [ ] in code\n```\n- [ ] real ^r\r\n";
        let texts: Vec<String> = parse_note(src).into_iter().map(|t| t.text).collect();
        assert_eq!(texts, ["real"]);
    }
}
