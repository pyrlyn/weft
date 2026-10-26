//! Removing an element by id. One SPEC §7 `remove` patch goes through `Editor::apply`. A rejection
//! leaves the document unchanged. An accepted remove marks the document dirty until a successful save.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use weft_studio::Editor;

const LOGIN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/login/screen.weft"
);

/// A directory of its own per test, removed afterwards. Nothing here writes into `corpus/`.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("weft-studio-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    /// Copies the login screen. The corpus file stays untouched.
    fn copy_login(&self) -> PathBuf {
        let path = self.0.join("screen.weft");
        std::fs::copy(LOGIN, &path).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn open_login(name: &str) -> (Scratch, PathBuf, Editor) {
    let scratch = Scratch::new(name);
    let file = scratch.copy_login();
    let editor = Editor::open(&file).unwrap();
    (scratch, file, editor)
}

fn has_id(editor: &Editor, id: &str) -> bool {
    editor
        .layers()
        .iter()
        .any(|layer| layer.id.as_deref() == Some(id))
}

#[test]
fn remove_drops_the_reset_link_and_marks_the_document_dirty() {
    let (_scratch, file, mut editor) = open_login("remove-reset");
    let original = std::fs::read(&file).unwrap();
    assert!(!editor.state().dirty);
    assert!(has_id(&editor, "reset"));
    assert!(has_id(&editor, "signup"));

    editor.remove("reset").unwrap();

    assert!(!has_id(&editor, "reset"));
    assert!(has_id(&editor, "signup"));
    assert!(editor.state().dirty);
    assert_eq!(std::fs::read(&file).unwrap(), original);
}

#[test]
fn save_drops_reset_and_a_reopen_is_clean() {
    let (_scratch, file, mut editor) = open_login("remove-save");
    editor.remove("reset").unwrap();

    editor.save(Some(&file)).unwrap();

    assert!(!editor.state().dirty);
    let again = Editor::open(&file).unwrap();
    assert!(!has_id(&again, "reset"));
    assert!(has_id(&again, "signup"));
    assert!(!again.state().dirty);
}

#[test]
fn an_unknown_id_leaves_the_document_and_dirty_flag() {
    let (_scratch, file, mut editor) = open_login("remove-reject");
    let original = std::fs::read(&file).unwrap();
    let layers = editor.layers();
    assert!(!editor.state().dirty);

    let rejected = editor.remove("no-such").unwrap_err();

    assert!(!rejected.is_empty());
    assert_eq!(editor.layers(), layers);
    assert!(!editor.state().dirty);
    assert_eq!(std::fs::read(&file).unwrap(), original);
}
