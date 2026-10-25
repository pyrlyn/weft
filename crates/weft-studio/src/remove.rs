//! Removing an element by id (T69.3e).

use weft_core::Diagnostic;

use super::Editor;

impl Editor {
    /// Removes the element whose id is `id` with one SPEC §7 `remove` patch through `Editor::apply`.
    /// A rejection leaves the document unchanged.
    pub fn remove(&mut self, id: &str) -> Result<(), Vec<Diagnostic>> {
        self.apply(&serde_json::json!([{ "op": "remove", "id": id }]))
    }
}
