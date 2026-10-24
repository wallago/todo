---
tags: [todo]
---

# Todo

Sample note for testing the parser, the TUI and the rest.
Lines use the Obsidian Tasks plugin format; "today" in these examples is 2026-10-24.

## Today

- [ ] Reply to Marc about the invoice 🔺 📅 2026-10-24 #work ^a1b2c3
- [ ] Buy milk 📅 2026-10-24 #errand ^d4e5f6
- [/] Write the md parser ⏫ ➕ 2026-10-22 📅 2026-10-25 #dev #todo-app ^g7h8i9

## Overdue

- [ ] Renew car insurance ⏫ 📅 2026-10-20 #admin ^j0k1l2
- [ ] Call the dentist 🔼 ➕ 2026-10-01 📅 2026-10-15 ^m3n4o5

## Upcoming

- [ ] Book train tickets for Christmas 🔼 📅 2026-11-15 #travel ^p6q7r8
- [ ] Sort out the garage 🔽 #home ^s9t0u1
- [ ] Learn ratatui layouts ⏬ #dev #learning ^v2w3x4
- [ ] Plan the desk dongle enclosure 📅 2026-12-01 #dev/hardware ^y5z6a7

## Done

- [x] Set up the Rust workspace ➕ 2026-10-20 ✅ 2026-10-24 #dev ^b8c9d0
- [X] Pay rent 📅 2026-10-05 ✅ 2026-10-03 #admin ^e1f2g3
- [-] Go to the Saturday market 📅 2026-10-18 #errand ^h4i5j6

## Edge cases

- [ ] Task with no id, no dates, no tags
- [ ] Read [[Obsidian Tasks]] notes and the [docs](https://publish.obsidian.md/tasks) ^k7l8m9
- [ ] Text that mentions `#not-a-tag` in code and an issue number #42 ^n0o1p2
- [ ] Emoji in the text 🎉 is not a field ^q3r4s5
- [ ] Fields in a different order 📅 2026-11-02 ⏫ #work ➕ 2026-10-23 ^t6u7v8
- [?] Custom status from another theme ^w9x0y1
- [ ] Parent task #home ^z2a3b4
    - [ ] Indented subtask ^c5d6e7
    - [x] Done subtask ✅ 2026-10-23 ^f8g9h0
* [ ] Star bullet instead of dash ^i1j2k3
1. [ ] Numbered list task ^l4m5n6
- [ ]    Extra spaces   around   the text    ^o7p8q9

Plain text and normal bullets must stay untouched:

- Not a task, just a bullet
- [] Broken checkbox, not a task
> - [ ] Task inside a quote ^r0s1t2

```
- [ ] Inside a code block, never a task
```
