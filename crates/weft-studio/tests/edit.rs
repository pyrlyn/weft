//! Applying a patch (SPEC §7) and the dirty flag. A rejected list changes nothing. An accepted
//! list replaces the document, and `dirty` follows the canonical markup until a successful save.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use weft_core::{ParseOptions, Value, has_errors, parse, parse_json, serialize};
use weft_studio::{Editor, Inspection};

const LOGIN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/login/screen.weft"
);

const SUBMIT: &str = "/screen#login/form#form/button#submit";

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

fn patches(text: &str) -> serde_json::Value {
    parse_json(text).unwrap()
}

/// The submit button's diagnostic path, confirmed against the layers list.
fn submit_path(editor: &Editor) -> String {
    let path = editor
        .layers()
        .into_iter()
        .find(|layer| layer.id.as_deref() == Some("submit"))
        .unwrap()
        .path;
    assert_eq!(path, SUBMIT);
    path
}

fn variant(editor: &Editor) -> Value {
    let path = submit_path(editor);
    editor
        .inspect(&path)
        .unwrap()
        .props
        .into_iter()
        .find_map(|(name, value)| (name == "variant").then_some(value))
        .unwrap()
}

fn readings(editor: &Editor) -> Vec<Inspection> {
    editor
        .layers()
        .iter()
        .map(|layer| editor.inspect(&layer.path).unwrap())
        .collect()
}

#[test]
fn a_set_patch_changes_the_variant_and_marks_the_document_dirty() {
    let (_scratch, file, mut editor) = open_login("edit-set");
    let original = std::fs::read(&file).unwrap();
    assert!(!editor.state().dirty);

    editor
        .apply(&patches(
            r#"[{"op":"set","id":"submit","prop":"variant","value":"danger"}]"#,
        ))
        .unwrap();

    assert_eq!(variant(&editor), Value::String("danger".into()));
    assert!(editor.state().dirty);
    assert!(!has_errors(&editor.state().diagnostics));
    assert_eq!(std::fs::read(&file).unwrap(), original);
}

#[test]
fn save_writes_the_canonical_variant_and_clears_dirty() {
    let (_scratch, file, mut editor) = open_login("edit-save");
    editor
        .apply(&patches(
            r#"[{"op":"set","id":"submit","prop":"variant","value":"danger"}]"#,
        ))
        .unwrap();

    editor.save(Some(&file)).unwrap();

    let saved = std::fs::read_to_string(&file).unwrap();
    let original = std::fs::read_to_string(LOGIN).unwrap();
    assert_eq!(
        saved,
        original.replace("variant=\"primary\"", "variant=\"danger\"")
    );
    assert_eq!(
        saved,
        serialize(&parse(&saved, &ParseOptions::default()).document.unwrap())
    );
    assert!(!editor.state().dirty);
    assert_eq!(variant(&editor), Value::String("danger".into()));

    let again = Editor::open(&file).unwrap();
    assert_eq!(variant(&again), Value::String("danger".into()));
    assert!(!again.state().dirty);
}

#[test]
fn an_unknown_id_leaves_the_document_diagnostics_and_dirty_flag() {
    let (_scratch, file, mut editor) = open_login("edit-reject");
    let original = std::fs::read(&file).unwrap();
    let layers = editor.layers();
    let seen = readings(&editor);
    let state = editor.state();
    assert!(!state.dirty);

    let rejected = editor
        .apply(&patches(
            r#"[{"op":"set","id":"no-such","prop":"variant","value":"danger"}]"#,
        ))
        .unwrap_err();

    assert!(!rejected.is_empty());
    assert_eq!(editor.layers(), layers);
    assert_eq!(readings(&editor), seen);
    let after = editor.state();
    assert_eq!(after.path, state.path);
    assert_eq!(after.title, state.title);
    assert_eq!(after.screen, state.screen);
    assert_eq!(after.dirty, state.dirty);
    assert!(!after.dirty);
    assert_eq!(after.diagnostics, state.diagnostics);
    assert_eq!(after.project, state.project);
    assert_eq!(after.project_diagnostics, state.project_diagnostics);
    assert_eq!(variant(&editor), Value::String("primary".into()));
    assert_eq!(std::fs::read(&file).unwrap(), original);
}

#[test]
fn setting_the_current_variant_stays_clean() {
    let (_scratch, file, mut editor) = open_login("edit-same");
    let original = std::fs::read(&file).unwrap();

    editor
        .apply(&patches(
            r#"[{"op":"set","id":"submit","prop":"variant","value":"primary"}]"#,
        ))
        .unwrap();

    assert!(!editor.state().dirty);
    assert_eq!(variant(&editor), Value::String("primary".into()));
    assert!(!has_errors(&editor.state().diagnostics));
    assert_eq!(std::fs::read(&file).unwrap(), original);
}

#[test]
fn an_empty_patch_list_stays_clean() {
    let (_scratch, file, mut editor) = open_login("edit-empty");
    let original = std::fs::read(&file).unwrap();
    let seen = readings(&editor);

    editor.apply(&patches("[]")).unwrap();

    assert!(!editor.state().dirty);
    assert_eq!(readings(&editor), seen);
    assert_eq!(variant(&editor), Value::String("primary".into()));
    assert_eq!(std::fs::read(&file).unwrap(), original);
}
