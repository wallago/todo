use ratatui::crossterm::event::{Event, KeyEvent};
use tui_input::{InputRequest, backend::crossterm::to_input_request};

use crate::{prelude::*, ui::prelude::*};

impl State {
    /// Feeds `key` to the open field: `leave` drops the edit, `confirm` saves
    /// it once it passes the field's rules, anything else edits the buffer.
    pub(super) fn on_edit_key(&mut self, key: KeyEvent) -> Result<()> {
        let dashboard = &mut self.dashboard;
        let Some(input) = &mut dashboard.editing else {
            return Ok(());
        };
        if self.keybindings.leave.matches(&key) {
            dashboard.editing = None;
        } else if self.keybindings.confirm.matches(&key)
            && let Some(field) = dashboard.row.field()
        {
            if field.validate(input.value()).is_ok() {
                input.value().clone_into(field.get_mut(&mut self.app));
                dashboard.editing = None;
                self.app.generate()?;
            }
        } else if let Some(request) = to_input_request(&Event::Key(key))
            && let Some(field) = dashboard.row.field()
        {
            // A char the field can't hold is dropped before it's ever shown.
            if let InputRequest::InsertChar(c) = request
                && !field.can_insert(input.value(), c)
            {
                return Ok(());
            }
            input.handle(request);
        }
        Ok(())
    }
}
