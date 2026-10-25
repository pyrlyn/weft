//! Undo and redo of an applied patch. The stack stores documents. A rejected list is not a step,
//! and a new change clears redo.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use weft_core::{Value, parse_json};
use weft_studio::{Editor, Session};

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

fn open_login(name: &str) -> (Scratch, Session) {
    let scratch = Scratch::new(name);
    let file = scratch.copy_login();
    let session = Session::open(&file).unwrap();
    (scratch, session)
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

const DANGER: &str = r#"[{"op":"set","id":"submit","prop":"variant","value":"danger"}]"#;
const SECONDARY: &str = r#"[{"op":"set","id":"submit","prop":"variant","value":"secondary"}]"#;

#[test]
fn undo_restores_the_opened_variant_and_redo_brings_the_change_back() {
    let (_scratch, mut session) = open_login("history-undo");

    session.apply(&patches(DANGER)).unwrap();

    assert_eq!(variant(session.editor()), Value::String("danger".into()));
    assert!(session.editor().state().dirty);

    assert!(session.undo());
    assert_eq!(variant(session.editor()), Value::String("primary".into()));
    assert!(!session.editor().state().dirty);

    assert!(session.redo());
    assert_eq!(variant(session.editor()), Value::String("danger".into()));
    assert!(session.editor().state().dirty);
}

#[test]
fn a_second_apply_clears_redo() {
    let (_scratch, mut session) = open_login("history-clear");
    session.apply(&patches(DANGER)).unwrap();
    assert!(session.undo());
    assert!(session.redo());

    session.apply(&patches(SECONDARY)).unwrap();

    assert!(!session.redo());
    assert!(session.undo());
    assert_eq!(variant(session.editor()), Value::String("danger".into()));
    assert!(session.editor().state().dirty);
}

#[test]
fn undo_on_a_fresh_session_returns_false() {
    let (_scratch, mut session) = open_login("history-fresh");

    assert!(!session.undo());
    assert_eq!(variant(session.editor()), Value::String("primary".into()));
    assert!(!session.editor().state().dirty);
}

#[test]
fn a_rejected_patch_leaves_the_undo_stack_empty() {
    let (_scratch, mut session) = open_login("history-reject");

    let rejected = session
        .apply(&patches(
            r#"[{"op":"set","id":"no-such","prop":"variant","value":"danger"}]"#,
        ))
        .unwrap_err();

    assert!(!rejected.is_empty());
    assert!(!session.undo());
    assert_eq!(variant(session.editor()), Value::String("primary".into()));
    assert!(!session.editor().state().dirty);
}
