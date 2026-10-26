//! Moving an element under a parent id (T69.3f).

use serde_json::json;
use weft_core::Diagnostic;

use crate::Editor;

impl Editor {
    /// Moves the element `id` under `parent` with one SPEC §7 `move` patch through `Editor::apply`.
    /// `slot` and `index` are omitted from the patch when `None`.
    pub fn relocate(
        &mut self,
        id: &str,
        parent: &str,
        slot: Option<&str>,
        index: Option<u64>,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut patch = json!({
            "op": "move",
            "id": id,
            "parent": parent,
        });
        if let Some(slot) = slot {
            patch["slot"] = json!(slot);
        }
        if let Some(index) = index {
            patch["index"] = json!(index);
        }
        self.apply(&json!([patch]))
    }
}
