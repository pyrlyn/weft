//! Binding a prop to a data path (T69.3g).

use weft_core::Diagnostic;

use super::Editor;

impl Editor {
    /// Binds `prop` of element `id` to the data path `path` with one SPEC §7 `set` patch
    /// through `Editor::apply`. The value is `{"bind": path}` with no `not`.
    pub fn bind(&mut self, id: &str, prop: &str, path: &str) -> Result<(), Vec<Diagnostic>> {
        self.apply(&serde_json::json!([{
            "op": "set",
            "id": id,
            "prop": prop,
            "value": { "bind": path }
        }]))
    }
}
