//! One task line: `- [ ] text ⏫ ➕ 2026-10-24 📅 2026-10-30 #tag ^id`.

use chrono::NaiveDate;
use todo_core::{Id, Priority, Status, Todo};

/// Date format used by the Tasks plugin.
const DATE: &str = "%Y-%m-%d";

/// Parses a task line into its prefix (indent and bullet, e.g. `"  - "`) and
/// the todo. Returns `None` for anything that is not a task.
///
/// A task without a `^id` gets an empty [`Id`].
#[must_use]
pub fn parse_line(line: &str) -> Option<(&str, Todo)> {
    let indent = line.len() - line.trim_start().len();
    let after_bullet = strip_bullet(&line[indent..])?;
    let rest = after_bullet.trim_start();
    let prefix = &line[..line.len() - rest.len()];

    let mut chars = rest.chars();
    let (Some('['), Some(mark), Some(']')) = (chars.next(), chars.next(), chars.next()) else {
        return None;
    };
    let body = chars.as_str();
    if !body.is_empty() && !body.starts_with(char::is_whitespace) {
        return None;
    }
    Some((prefix, parse_body(Status::from_char(mark), body)))
}

/// Writes a todo as a task line, after `prefix` (use `"- "` for a new task).
#[must_use]
pub fn write_line(prefix: &str, todo: &Todo) -> String {
    let mut line = format!("{prefix}[{}]", todo.status.as_char());
    let mut push = |part: &str| {
        if !part.is_empty() {
            line.push(' ');
            line.push_str(part);
        }
    };
    push(&todo.text);
    push(todo.priority.emoji().unwrap_or_default());
    for (emoji, date) in [("➕", todo.created), ("📅", todo.due), ("✅", todo.done)] {
        if let Some(date) = date {
            push(&format!("{emoji} {date}"));
        }
    }
    for tag in &todo.tags {
        push(&format!("#{tag}"));
    }
    if !todo.id.0.is_empty() {
        push(&format!("^{}", todo.id.0));
    }
    line
}

/// Strips a list bullet (`-`, `*`, `+`, `1.`, `1)`) and the space after it.
fn strip_bullet(s: &str) -> Option<&str> {
    let rest = match s.chars().next()? {
        '-' | '*' | '+' => &s[1..],
        '0'..='9' => s
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .strip_prefix(['.', ')'])?,
        _ => return None,
    };
    rest.starts_with(char::is_whitespace).then_some(rest)
}

/// Reads fields from the end of the line, the way the Tasks plugin does, and
/// stops at the first word that isn't one. What's left is the text.
fn parse_body(status: Status, body: &str) -> Todo {
    let mut rest = body;
    let mut id = Id(String::new());
    if let Some((head, word)) = split_last(rest)
        && let Some(block) = word.strip_prefix('^')
        && is_block_id(block)
    {
        id = Id(block.to_owned());
        rest = head;
    }

    let mut priority = None;
    let (mut due, mut created, mut done) = (None, None, None);
    let mut tags = Vec::new();
    while let Some((head, word)) = split_last(rest) {
        if priority.is_none()
            && let Some(p) = priority_from_emoji(word)
        {
            priority = Some(p);
        } else if let Some(tag) = parse_tag(word) {
            tags.push(tag.to_owned());
        } else if let Ok(date) = NaiveDate::parse_from_str(word, DATE)
            && let Some((head, emoji)) = split_last(head)
        {
            let slot = match emoji {
                "📅" | "📆" | "🗓" | "🗓\u{fe0f}" => &mut due,
                "➕" => &mut created,
                "✅" => &mut done,
                _ => break,
            };
            if slot.is_some() {
                break;
            }
            *slot = Some(date);
            rest = head;
            continue;
        } else {
            break;
        }
        rest = head;
    }
    tags.reverse();

    Todo {
        id,
        text: rest.trim().to_owned(),
        status,
        priority: priority.unwrap_or_default(),
        due,
        created,
        done,
        tags,
    }
}

/// Splits off the last whitespace-separated word.
fn split_last(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_end();
    if s.is_empty() {
        return None;
    }
    Some(match s.rfind(char::is_whitespace) {
        Some(i) => (&s[..i], s[i..].trim_start()),
        None => ("", s),
    })
}

/// Obsidian block ids: letters, digits and dashes.
fn is_block_id(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Obsidian tags: `#` then letters, digits, `_`, `-`, `/`, not only digits.
fn parse_tag(word: &str) -> Option<&str> {
    let tag = word.strip_prefix('#')?;
    let valid = tag
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'));
    (valid && tag.chars().any(|c| !c.is_ascii_digit())).then_some(tag)
}

/// Reverse of [`Priority::emoji`].
fn priority_from_emoji(word: &str) -> Option<Priority> {
    [
        Priority::Lowest,
        Priority::Low,
        Priority::Medium,
        Priority::High,
        Priority::Highest,
    ]
    .into_iter()
    .find(|p| p.emoji() == Some(word))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn reads_every_field() -> Result<(), &'static str> {
        let line =
            "- [/] Write the md parser ⏫ ➕ 2026-10-22 📅 2026-10-25 #dev #todo-app ^g7h8i9";
        let (prefix, todo) = parse_line(line).ok_or("not a task")?;
        assert_eq!(prefix, "- ");
        assert_eq!(todo.id, Id("g7h8i9".into()));
        assert_eq!(todo.text, "Write the md parser");
        assert_eq!(todo.status, Status::InProgress);
        assert_eq!(todo.priority, Priority::High);
        assert_eq!(todo.created, NaiveDate::from_ymd_opt(2026, 10, 22));
        assert_eq!(todo.due, NaiveDate::from_ymd_opt(2026, 10, 25));
        assert_eq!(todo.tags, ["dev", "todo-app"]);
        assert_eq!(write_line(prefix, &todo), line);
        Ok(())
    }

    #[test]
    fn ignores_non_tasks() {
        for line in [
            "",
            "Plain text",
            "- Not a task",
            "- [] Broken checkbox",
            "-[ ] No space after the bullet",
            "> - [ ] Inside a quote",
            "- [ ]text glued to the box",
        ] {
            assert_eq!(parse_line(line), None, "{line}");
        }
    }

    #[test]
    fn numbers_and_hashes_in_text_stay_text() -> Result<(), &'static str> {
        let (_, todo) = parse_line("- [ ] Fix issue #42 by 2026-10-30").ok_or("not a task")?;
        assert_eq!(todo.text, "Fix issue #42 by 2026-10-30");
        assert!(todo.tags.is_empty());
        assert_eq!(todo.due, None);
        Ok(())
    }

    /// Any todo with ordinary text survives write then parse unchanged.
    fn arb_todo() -> impl Strategy<Value = Todo> {
        let status = prop_oneof![
            Just(Status::Todo),
            Just(Status::InProgress),
            Just(Status::Done),
            Just(Status::Cancelled),
        ];
        let priority = prop_oneof![
            Just(Priority::Lowest),
            Just(Priority::Low),
            Just(Priority::Normal),
            Just(Priority::Medium),
            Just(Priority::High),
            Just(Priority::Highest),
        ];
        let date = (2000i32..2100, 1u32..=12, 1u32..=28)
            .prop_map(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d).unwrap_or_default());
        (
            "([a-z0-9]{6})?",
            "[A-Za-z]([A-Za-z0-9 ,.!?']{0,30}[A-Za-z.!?])?",
            status,
            priority,
            proptest::option::of(date.clone()),
            proptest::option::of(date.clone()),
            proptest::option::of(date),
            proptest::collection::vec("[a-z][a-z0-9/_-]{0,8}", 0..3),
        )
            .prop_map(
                |(id, text, status, priority, due, created, done, tags)| Todo {
                    id: Id(id),
                    text,
                    status,
                    priority,
                    due,
                    created,
                    done,
                    tags,
                },
            )
    }

    proptest! {
        #[test]
        fn write_then_parse_round_trips(todo in arb_todo(), indent in "[ \t]{0,4}") {
            let prefix = format!("{indent}- ");
            let line = write_line(&prefix, &todo);
            prop_assert_eq!(parse_line(&line), Some((prefix.as_str(), todo)));
        }
    }
}
