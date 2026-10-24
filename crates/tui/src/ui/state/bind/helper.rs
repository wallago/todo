use crate::ui::prelude::*;

impl State {
    /// Returns the key bindings.
    pub(crate) fn get_key_bindings(&self) -> Vec<(String, &'static str)> {
        let mut binds = Vec::new();
        if self.focus == Focus::Sidebar {
            binds.push((self.keybindings.scroll_up.to_string(), "Scroll UP"));
            binds.push((self.keybindings.scroll_down.to_string(), "Scroll DOWN"));
        } else {
            match self.mode {
                Mode::Dashboard => {
                    binds.push((self.keybindings.leave.to_string(), "Leave"));
                    binds.push((self.keybindings.confirm.to_string(), "Confirm"));
                    binds.push((self.keybindings.scroll_up.to_string(), "Scroll UP"));
                    binds.push((self.keybindings.scroll_down.to_string(), "Scroll DOWN"));
                    if self.dashboard.focus == DashboardPane::Options {
                        binds.push((self.keybindings.enter.to_string(), "Toogle opt"));
                    } else if self.dashboard.picker.is_some() {
                        binds.push((self.keybindings.scroll_left.to_string(), "Move to parent"));
                    }
                }
                Mode::Details => {
                    binds.push((self.keybindings.leave.to_string(), "Leave"));
                    binds.push((self.keybindings.confirm.to_string(), "Confirm"));
                    binds.push((self.keybindings.scroll_up.to_string(), "Scroll UP"));
                    binds.push((self.keybindings.scroll_down.to_string(), "Scroll DOWN"));
                }
                Mode::Settings => {
                    binds.push((self.keybindings.leave.to_string(), "Leave"));
                    binds.push((self.keybindings.confirm.to_string(), "Confirm"));
                }
            }
        }
        binds.push((self.keybindings.quit.to_string(), "Quit"));
        binds.push((self.keybindings.create.to_string(), "Create"));
        binds
    }
}
