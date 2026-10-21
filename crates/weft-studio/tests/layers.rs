//! The layers list and the inspector read, over the login screen and every corpus screen.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use weft_core::{ParseOptions, Value, element_paths, parse};
use weft_studio::Editor;

const LOGIN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/login/screen.weft"
);

fn corpus() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut screens: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path().join("screen.weft"))
        .filter(|path| path.is_file())
        .collect();
    screens.sort();
    screens
}

#[test]
fn login_layers_are_the_diagnostic_paths_in_document_order() {
    let editor = Editor::open(LOGIN.as_ref()).unwrap();
    let layers = editor.layers();
    let paths: Vec<&str> = layers.iter().map(|layer| layer.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "/screen#login",
            "/screen#login/form#form",
            "/screen#login/form#form/heading#title",
            "/screen#login/form#form/stack#fields",
            "/screen#login/form#form/stack#fields/field#email",
            "/screen#login/form#form/stack#fields/field#password",
            "/screen#login/form#form/button#submit",
            "/screen#login/form#form/slot[footer]/link#reset",
            "/screen#login/form#form/slot[footer]/link#signup",
        ]
    );
    assert_eq!(layers[0].depth, 0);
    assert_eq!(layers[0].kind, "screen");
    assert_eq!(layers[1].depth, 1);
    assert_eq!(layers[4].depth, 3);
    for layer in layers.iter().skip(1) {
        assert!(
            layers[..layers
                .iter()
                .position(|row| row.path == layer.path)
                .unwrap()]
                .iter()
                .any(
                    |earlier| layer.path.starts_with(&format!("{}/", earlier.path))
                        || layer.path.contains("/slot[")
                ),
            "{}",
            layer.path
        );
    }
}

#[test]
fn inspecting_login_reads_props_events_and_children() {
    let editor = Editor::open(LOGIN.as_ref()).unwrap();
    let form = editor.inspect("/screen#login/form#form").unwrap();
    assert_eq!(form.kind, "form");
    assert_eq!(form.id.as_deref(), Some("form"));
    assert_eq!(
        form.on,
        vec![("submit".to_owned(), "auth.submit".to_owned())]
    );
    assert_eq!(
        form.children,
        [
            "/screen#login/form#form/heading#title",
            "/screen#login/form#form/stack#fields",
            "/screen#login/form#form/button#submit",
            "/screen#login/form#form/slot[footer]/link#reset",
            "/screen#login/form#form/slot[footer]/link#signup",
        ]
    );

    let email = editor
        .inspect("/screen#login/form#form/stack#fields/field#email")
        .unwrap();
    assert_eq!(
        email.props.iter().find(|(name, _)| name == "value"),
        Some(&(
            "value".to_owned(),
            Value::Bind {
                bind: "$.email".into(),
                not: false,
            }
        ))
    );
    assert!(email.children.is_empty());

    let submit = editor
        .inspect("/screen#login/form#form/button#submit")
        .unwrap();
    assert_eq!(
        submit.props.iter().find(|(name, _)| name == "disabled"),
        Some(&(
            "disabled".to_owned(),
            Value::Bind {
                bind: "$.email".into(),
                not: true,
            }
        ))
    );
    assert!(editor.inspect("/screen#login/missing").is_none());
    assert!(editor.inspect("").is_none());
}

#[test]
fn every_corpus_screen_lists_the_same_elements_as_the_diagnostics() {
    for screen in corpus() {
        let markup = std::fs::read_to_string(&screen).unwrap();
        let document = parse(&markup, &ParseOptions::default()).document.unwrap();
        let editor = Editor::open(&screen).unwrap();
        let mut listed: Vec<String> = editor
            .layers()
            .into_iter()
            .map(|layer| layer.path)
            .collect();
        let mut named: Vec<String> = element_paths(&document)
            .into_iter()
            .map(|(_, path)| path)
            .collect();
        listed.sort();
        named.sort();
        assert_eq!(listed, named, "{}", screen.display());
        let layers = editor.layers();
        for window in layers.windows(2) {
            if window[1].path.starts_with(&format!("{}/", window[0].path)) {
                assert!(window[1].depth > window[0].depth);
            }
        }
    }
}
