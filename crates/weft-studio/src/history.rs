//! Undo and redo of an applied patch list (T69.3h). The stack stores documents, not inverse
//! patches. A rejected list leaves both stacks as they were. A change that alters the canonical
//! markup pushes the previous document and clears redo.

use std::path::Path;

use serde_json::Value as Json;
use weft_core::{Diagnostic, Document, serialize};

use crate::{Editor, EditorError};

/// One open screen and the documents an edit can return to.
pub struct Session {
    editor: Editor,
    undo: Vec<Document>,
    redo: Vec<Document>,
    /// Canonical markup captured at [`open`](Self::open). Undo and redo set `dirty` from it.
    opened: String,
}

impl Session {
    /// Opens the markup at `path` the way [`Editor::open`] does, and remembers its canonical markup.
    pub fn open(path: &Path) -> Result<Session, EditorError> {
        let editor = Editor::open(path)?;
        let opened = serialize(editor.document());
        Ok(Session {
            editor,
            undo: Vec::new(),
            redo: Vec::new(),
            opened,
        })
    }

    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    /// Applies patches. When the canonical markup changes, the previous document is pushed on
    /// the undo stack and the redo stack is cleared. A rejection leaves both stacks as they were.
    pub fn apply(&mut self, patches: &Json) -> Result<(), Vec<Diagnostic>> {
        let previous = self.editor.document_clone();
        let before = serialize(&previous);
        self.editor.apply(patches)?;
        if serialize(self.editor.document()) != before {
            self.undo.push(previous);
            self.redo.clear();
        }
        Ok(())
    }

    /// Restores the previous document. `dirty` is true when the restored markup differs from
    /// the markup captured at [`open`](Self::open). Returns false when the undo stack is empty.
    pub fn undo(&mut self) -> bool {
        exchange(
            &mut self.editor,
            &mut self.undo,
            &mut self.redo,
            &self.opened,
        )
    }

    /// Restores the document that [`undo`](Self::undo) put aside. `dirty` follows the same rule.
    /// Returns false when the redo stack is empty.
    pub fn redo(&mut self) -> bool {
        exchange(
            &mut self.editor,
            &mut self.redo,
            &mut self.undo,
            &self.opened,
        )
    }
}

/// Pops one document from `source` into the editor and pushes the editor's document onto `dest`.
fn exchange(
    editor: &mut Editor,
    source: &mut Vec<Document>,
    dest: &mut Vec<Document>,
    opened: &str,
) -> bool {
    let Some(document) = source.pop() else {
        return false;
    };
    dest.push(editor.document_clone());
    let dirty = serialize(&document) != opened;
    editor.replace_document(document, dirty);
    true
}
