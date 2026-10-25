//! Inserting an element under a parent id (T69.3d). One SPEC §7 `insert` goes through
//! `Editor::apply`. A rejected parent leaves the document and the dirty flag alone.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use weft_studio::Editor;

const LOGIN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/login/screen.weft"
);

const NOTE: &str = r#"<text id="note">Hi</text>"#;

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

fn layer_ids(editor: &Editor) -> Vec<Option<String>> {
    editor.layers().into_iter().map(|layer| layer.id).collect()
}

#[test]
fn insert_at_the_start_of_form_lists_the_text_before_the_heading_and_stays_on_disk() {
    let (_scratch, file, mut editor) = open_login("insert-note");
    let original = std::fs::read(&file).unwrap();
    assert!(!editor.state().dirty);

    editor.insert("form", Some(0), NOTE).unwrap();

    let ids = layer_ids(&editor);
    let note = ids
        .iter()
        .position(|id| id.as_deref() == Some("note"))
        .unwrap();
    let heading = ids
        .iter()
        .position(|id| id.as_deref() == Some("title"))
        .unwrap();
    assert!(note < heading);
    let layers = editor.layers();
    assert_eq!(layers[note].kind, "text");
    assert_eq!(layers[note].path, "/screen#login/form#form/text#note");
    assert!(editor.state().dirty);
    assert_eq!(std::fs::read(&file).unwrap(), original);
}

#[test]
fn save_then_open_keeps_the_inserted_text_and_clears_dirty() {
    let (_scratch, file, mut editor) = open_login("insert-save");
    editor.insert("form", Some(0), NOTE).unwrap();

    editor.save(Some(&file)).unwrap();

    let again = Editor::open(&file).unwrap();
    let ids = layer_ids(&again);
    let note = ids
        .iter()
        .position(|id| id.as_deref() == Some("note"))
        .unwrap();
    let heading = ids
        .iter()
        .position(|id| id.as_deref() == Some("title"))
        .unwrap();
    assert!(note < heading);
    assert_eq!(again.layers()[note].kind, "text");
    assert!(!again.state().dirty);
}

#[test]
fn an_unknown_parent_leaves_the_document_and_the_dirty_flag() {
    let (_scratch, file, mut editor) = open_login("insert-reject");
    let original = std::fs::read(&file).unwrap();
    let layers = editor.layers();
    assert!(!editor.state().dirty);

    let rejected = editor.insert("no-such", None, NOTE).unwrap_err();

    assert!(!rejected.is_empty());
    assert_eq!(editor.layers(), layers);
    assert!(!editor.state().dirty);
    assert_eq!(std::fs::read(&file).unwrap(), original);
}
