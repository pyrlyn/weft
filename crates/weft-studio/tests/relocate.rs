//! Moving an element (SPEC §7 `move`) through `Editor::relocate`. A rejected move changes
//! nothing. An accepted move reorders the document, and `dirty` follows the canonical markup
//! until a successful save.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use weft_core::{Code, has_errors};
use weft_studio::{Editor, Inspection};

const LOGIN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/login/screen.weft"
);

/// The login screen in document order. The footer slot of `form` is `reset`, then `signup`.
const LOGIN_ORDER: &[&str] = &[
    "/screen#login",
    "/screen#login/form#form",
    "/screen#login/form#form/heading#title",
    "/screen#login/form#form/stack#fields",
    "/screen#login/form#form/stack#fields/field#email",
    "/screen#login/form#form/stack#fields/field#password",
    "/screen#login/form#form/button#submit",
    "/screen#login/form#form/slot[footer]/link#reset",
    "/screen#login/form#form/slot[footer]/link#signup",
];

/// `signup` moved to index 0 of `form`'s `footer` slot, so it stands before `reset`.
const MOVED_ORDER: &[&str] = &[
    "/screen#login",
    "/screen#login/form#form",
    "/screen#login/form#form/heading#title",
    "/screen#login/form#form/stack#fields",
    "/screen#login/form#form/stack#fields/field#email",
    "/screen#login/form#form/stack#fields/field#password",
    "/screen#login/form#form/button#submit",
    "/screen#login/form#form/slot[footer]/link#signup",
    "/screen#login/form#form/slot[footer]/link#reset",
];

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

fn layer_paths(editor: &Editor) -> Vec<String> {
    editor
        .layers()
        .into_iter()
        .map(|layer| layer.path)
        .collect()
}

fn readings(editor: &Editor) -> Vec<Inspection> {
    editor
        .layers()
        .iter()
        .map(|layer| editor.inspect(&layer.path).unwrap())
        .collect()
}

/// `signup` before `reset` in `layers()`.
fn signup_before_reset(editor: &Editor) -> bool {
    let paths = layer_paths(editor);
    let signup = paths.iter().position(|path| path.ends_with("link#signup"));
    let reset = paths.iter().position(|path| path.ends_with("link#reset"));
    signup
        .zip(reset)
        .is_some_and(|(signup, reset)| signup < reset)
}

#[test]
fn moving_signup_to_the_footer_start_puts_it_before_reset_and_marks_dirty() {
    let (_scratch, file, mut editor) = open_login("relocate-order");
    let original = std::fs::read(&file).unwrap();
    assert_eq!(layer_paths(&editor), LOGIN_ORDER);
    assert!(!editor.state().dirty);
    assert!(!signup_before_reset(&editor));

    // Legal move inside the login screen: `signup` to index 0 of `form`'s `footer` slot.
    editor
        .relocate("signup", "form", Some("footer"), Some(0))
        .unwrap();

    assert_eq!(layer_paths(&editor), MOVED_ORDER);
    assert!(signup_before_reset(&editor));
    assert!(editor.state().dirty);
    assert!(!has_errors(&editor.state().diagnostics));
    assert_eq!(std::fs::read(&file).unwrap(), original);
}

#[test]
fn save_keeps_signup_before_reset_and_clears_dirty() {
    let (_scratch, file, mut editor) = open_login("relocate-save");
    let original = std::fs::read(&file).unwrap();
    editor
        .relocate("signup", "form", Some("footer"), Some(0))
        .unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), original);

    editor.save(Some(&file)).unwrap();

    assert!(!editor.state().dirty);
    assert_eq!(layer_paths(&editor), MOVED_ORDER);
    assert!(signup_before_reset(&editor));
    let saved = std::fs::read(&file).unwrap();
    assert_ne!(saved, original);
    let text = String::from_utf8(saved).unwrap();
    let signup = text.find("id=\"signup\"").unwrap();
    let reset = text.find("id=\"reset\"").unwrap();
    assert!(signup < reset);

    let again = Editor::open(&file).unwrap();
    assert_eq!(layer_paths(&again), MOVED_ORDER);
    assert!(signup_before_reset(&again));
    assert!(!again.state().dirty);
}

#[test]
fn an_unknown_id_leaves_the_document_and_dirty_flag() {
    let (_scratch, file, mut editor) = open_login("relocate-missing");
    let original = std::fs::read(&file).unwrap();
    let paths = layer_paths(&editor);
    let seen = readings(&editor);
    let state = editor.state();
    assert!(!state.dirty);
    assert_eq!(paths, LOGIN_ORDER);

    let rejected = editor.relocate("no-such", "form", None, None).unwrap_err();

    assert!(
        rejected
            .iter()
            .any(|diagnostic| diagnostic.code == Code::W502)
    );
    assert_eq!(layer_paths(&editor), paths);
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
    assert_eq!(std::fs::read(&file).unwrap(), original);
}
