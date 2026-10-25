//! Binding a prop to a data path. One SPEC §7 `set` patch goes through `Editor::apply`.
//! A rejected id changes nothing. The file on disk stays as it was until a successful save.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use weft_core::Value;
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

fn disabled(editor: &Editor) -> Value {
    editor
        .inspect(SUBMIT)
        .unwrap()
        .props
        .into_iter()
        .find_map(|(name, value)| (name == "disabled").then_some(value))
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
fn bind_sets_disabled_and_marks_the_document_dirty() {
    let (_scratch, file, mut editor) = open_login("bind-set");
    let original = std::fs::read(&file).unwrap();
    assert!(!editor.state().dirty);

    editor.bind("submit", "disabled", "$.busy").unwrap();

    assert_eq!(
        disabled(&editor),
        Value::Bind {
            bind: "$.busy".into(),
            not: false,
        }
    );
    assert!(editor.state().dirty);
    assert_eq!(std::fs::read(&file).unwrap(), original);
}

#[test]
fn save_and_reopen_keeps_the_binding_and_clears_dirty() {
    let (_scratch, file, mut editor) = open_login("bind-save");
    editor.bind("submit", "disabled", "$.busy").unwrap();

    editor.save(Some(&file)).unwrap();

    assert!(!editor.state().dirty);
    assert_eq!(
        disabled(&editor),
        Value::Bind {
            bind: "$.busy".into(),
            not: false,
        }
    );

    let again = Editor::open(&file).unwrap();
    assert_eq!(
        disabled(&again),
        Value::Bind {
            bind: "$.busy".into(),
            not: false,
        }
    );
    assert!(!again.state().dirty);
}

#[test]
fn an_unknown_id_leaves_the_document_and_dirty_flag() {
    let (_scratch, file, mut editor) = open_login("bind-reject");
    let original = std::fs::read(&file).unwrap();
    let layers = editor.layers();
    let seen = readings(&editor);
    assert!(!editor.state().dirty);
    assert_eq!(
        disabled(&editor),
        Value::Bind {
            bind: "$.email".into(),
            not: true,
        }
    );

    let rejected = editor.bind("no-such", "disabled", "$.busy").unwrap_err();

    assert!(!rejected.is_empty());
    assert_eq!(editor.layers(), layers);
    assert_eq!(readings(&editor), seen);
    assert!(!editor.state().dirty);
    assert_eq!(
        disabled(&editor),
        Value::Bind {
            bind: "$.email".into(),
            not: true,
        }
    );
    assert_eq!(std::fs::read(&file).unwrap(), original);
}
