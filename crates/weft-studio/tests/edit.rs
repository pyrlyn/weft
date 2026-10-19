//! Patches on an open screen (SPEC §7): a rejected list changes nothing, undo and redo restore
//! the documents on either side, and a save is what `dirty` compares against.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use serde_json::json;
use weft_studio::{EDIT_HISTORY, Editor, EditorError};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("weft-edit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn screen(body: &str) -> String {
    format!(r#"<screen id="s" weft="0.3" label="Hi">{body}</screen>"#)
}

fn open(scratch: &Scratch, body: &str) -> Editor {
    let path = scratch.write("screen.weft", &screen(body));
    Editor::open(&path).unwrap()
}

#[test]
fn an_edit_is_recorded_and_undo_restores_the_file() {
    let scratch = Scratch::new("set");
    let mut editor = open(&scratch, r#"<text id="t">Hi</text>"#);
    let opened = editor.markup();
    assert!(!editor.state().dirty);
    let patches = json!([{"op": "set", "id": "t", "prop": "text", "value": "Bye"}]);
    let report = editor.edit(&patches);
    assert!(report.applied && report.changed, "{report:?}");
    assert!(report.diagnostics.is_empty(), "{report:?}");
    assert!(editor.state().dirty);
    assert_eq!(editor.edits(), vec![&patches]);
    assert!(editor.markup().contains(">Bye</text>"));

    assert!(editor.undo());
    assert!(!editor.state().dirty);
    assert!(editor.edits().is_empty());
    assert_eq!(editor.markup(), opened);
    assert!(editor.redo());
    assert!(editor.state().dirty);
    assert!(editor.markup().contains(">Bye</text>"));

    editor.save(None).unwrap();
    assert!(!editor.state().dirty);
    assert_eq!(
        std::fs::read_to_string(editor.state().path).unwrap(),
        editor.markup()
    );
}

#[test]
fn a_rejected_list_and_a_no_op_leave_no_edit() {
    let scratch = Scratch::new("reject");
    let mut editor = open(&scratch, r#"<text id="t">Hi</text>"#);
    let missing = editor.edit(&json!([{"op": "set", "id": "gone", "prop": "text", "value": "x"}]));
    assert!(!missing.applied && !missing.changed);
    assert!(!missing.diagnostics.is_empty());
    assert!(!editor.state().dirty);
    assert!(!editor.undo());

    let same = editor.edit(&json!([{"op": "set", "id": "t", "prop": "text", "value": "Hi"}]));
    assert!(same.applied && !same.changed, "{same:?}");
    assert!(!editor.undo());

    let empty = editor.edit(&json!([]));
    assert!(empty.applied && !empty.changed);
    let shaped = editor.edit(&json!({"op": "set"}));
    assert!(!shaped.applied);
    assert!(editor.markup().contains(">Hi</text>"));
}

#[test]
fn a_new_edit_drops_redo() {
    let scratch = Scratch::new("redo");
    let mut editor = open(&scratch, r#"<text id="t">Hi</text>"#);
    editor.edit(&json!([{"op": "set", "id": "t", "prop": "text", "value": "Bye"}]));
    assert!(editor.undo());
    editor.edit(&json!([{"op": "set", "id": "t", "prop": "text", "value": "Later"}]));
    assert!(!editor.redo());
    assert!(editor.markup().contains(">Later</text>"));
}

#[test]
fn insert_then_undo_is_the_screen_that_opened() {
    let scratch = Scratch::new("insert");
    let mut editor = open(&scratch, r#"<text id="t">Hi</text>"#);
    let opened = editor.markup();
    let report = editor.edit(&json!([{
        "op": "insert",
        "parent": "s",
        "markup": "<button id=\"go\">Go</button>"
    }]));
    assert!(report.changed, "{report:?}");
    assert!(editor.markup().contains("button id=\"go\""));
    assert!(editor.undo());
    assert_eq!(editor.markup(), opened);
}

#[test]
fn the_project_actions_refuse_an_unknown_action() {
    let scratch = Scratch::new("actions");
    scratch.write("weft.json", r#"{ "actions": ["save"] }"#);
    let path = scratch.write(
        "screen.weft",
        &screen(r#"<button id="go" on-press="save">Go</button>"#),
    );
    let mut editor = Editor::open(&path).unwrap();
    let refused =
        editor.edit(&json!([{"op": "set", "id": "go", "prop": "on-press", "value": "other"}]));
    assert!(!refused.applied, "{refused:?}");
    assert!(!editor.state().dirty);
    let kept = editor.edit(&json!([{"op": "set", "id": "go", "prop": "text", "value": "Save"}]));
    assert!(kept.changed, "{kept:?}");
    assert!(editor.markup().contains(">Save</button>"));
}

#[test]
fn a_result_that_does_not_validate_is_refused() {
    let scratch = Scratch::new("invalid");
    let mut editor = open(&scratch, r#"<heading id="t" level="1">Hi</heading>"#);
    let report = editor.edit(&json!([{"op": "set", "id": "t", "prop": "level", "value": null}]));
    assert!(!report.applied, "{report:?}");
    assert!(!editor.state().dirty);
    assert!(editor.markup().contains(r#"level="1""#));
}

#[test]
fn history_keeps_the_newest_edits_only() {
    let scratch = Scratch::new("cap");
    let mut editor = open(&scratch, r#"<text id="t">Hi</text>"#);
    for n in 0..=EDIT_HISTORY {
        let report = editor.edit(&json!([{
            "op": "set",
            "id": "s",
            "prop": "label",
            "value": n.to_string()
        }]));
        assert!(report.changed, "{n} {report:?}");
    }
    assert_eq!(editor.edits().len(), EDIT_HISTORY);
    for _ in 0..EDIT_HISTORY {
        assert!(editor.undo());
    }
    assert!(!editor.undo());
    let saved = editor.markup();
    assert!(saved.contains("label=\"0\""), "{saved}");
    assert!(!saved.contains("label=\"Hi\""));
}

#[test]
fn every_corpus_screen_undoes_a_label_edit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut screens: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path().join("screen.weft"))
        .filter(|path| path.is_file())
        .collect();
    screens.sort();
    let scratch = Scratch::new("corpus");
    for screen in screens {
        let original = std::fs::read(&screen).unwrap();
        let mut editor = Editor::open(&screen).unwrap();
        let Some(id) = editor.state().screen.clone() else {
            continue;
        };
        let report = editor.edit(&json!([{
            "op": "set",
            "id": id,
            "prop": "label",
            "value": "Edited by the test"
        }]));
        assert!(report.changed, "{} {report:?}", screen.display());
        assert!(editor.undo());
        assert!(!editor.state().dirty);
        let copy = scratch
            .0
            .join(screen.parent().unwrap().file_name().unwrap());
        editor.save(Some(&copy)).unwrap();
        assert_eq!(
            std::fs::read(&copy).unwrap(),
            original,
            "{}",
            screen.display()
        );
    }
}

#[test]
fn a_failed_save_keeps_the_edit_and_the_dirty_flag() {
    let scratch = Scratch::new("save");
    let mut editor = open(&scratch, r#"<text id="t">Hi</text>"#);
    editor.edit(&json!([{"op": "set", "id": "t", "prop": "text", "value": "Bye"}]));
    let blocked = scratch.write("taken.weft/keep.txt", "kept");
    let error = editor
        .save(Some(&scratch.0.join("taken.weft")))
        .unwrap_err();
    assert!(matches!(error, EditorError::Write(_)), "{error:?}");
    assert_eq!(std::fs::read_to_string(blocked).unwrap(), "kept");
    assert!(editor.state().dirty);
    assert_eq!(editor.edits().len(), 1);
}
