//! Inserting an element under a parent id (T69.3d).

use serde_json::{Map, Value as Json};
use weft_core::Diagnostic;

use super::Editor;

impl Editor {
    /// Inserts `markup` as a child of the element whose id is `parent`.
    /// `index` is the child position; `None` appends. The patch is SPEC §7 `insert`
    /// and goes through `Editor::apply`, so a rejection leaves the document unchanged
    /// and a real change marks it dirty.
    pub fn insert(
        &mut self,
        parent: &str,
        index: Option<u64>,
        markup: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut patch = Map::new();
        patch.insert("op".to_owned(), Json::from("insert"));
        patch.insert("parent".to_owned(), Json::from(parent));
        patch.insert("markup".to_owned(), Json::from(markup));
        if let Some(index) = index {
            patch.insert("index".to_owned(), Json::from(index));
        }
        self.apply(&Json::Array(vec![Json::Object(patch)]))
    }
}
