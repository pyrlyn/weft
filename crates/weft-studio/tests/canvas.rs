//! Frames, the view and keyboard selection over the element tree. No Slint: a point hits a
//! frame, and focus walks diagnostic paths.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use weft_studio::{Canvas, EditorError, Focus, FrameGeom};

const LOGIN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/login/screen.weft"
);
const MENU: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/menu/screen.weft");

fn near(left: f64, right: f64) {
    assert!((left - right).abs() < 1e-9, "{left} != {right}");
}

fn login_paths() -> Vec<&'static str> {
    vec![
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
}

#[test]
fn frames_open_to_the_right_and_a_refused_file_adds_nothing() {
    let mut canvas = Canvas::new();
    assert!(canvas.is_empty());
    let first = canvas.open_frame(LOGIN.as_ref()).unwrap();
    let second = canvas.open_frame(MENU.as_ref()).unwrap();
    assert_eq!((first, second, canvas.len()), (0, 1, 2));
    let (_, geom) = canvas.frame(0).unwrap();
    assert_eq!(
        geom,
        FrameGeom {
            x: 0.0,
            y: 0.0,
            width: 390.0,
            height: 800.0,
        }
    );
    let (path, geom) = canvas.frame(1).unwrap();
    assert_eq!(path, PathBuf::from(MENU));
    assert_eq!(geom.x, 470.0);
    let missing = canvas.open_frame("/no/such/screen.weft".as_ref());
    assert!(matches!(missing, Err(EditorError::Read(_, _))));
    assert_eq!(canvas.len(), 2);
}

#[test]
fn zoom_keeps_the_point_under_the_cursor_and_stays_in_range() {
    let mut canvas = Canvas::new();
    assert!(canvas.zoom_at(2.0, 100.0, 40.0));
    near(canvas.zoom(), 2.0);
    near(canvas.pan().0, 100.0 - 100.0 * 2.0);
    near(canvas.pan().1, 40.0 - 40.0 * 2.0);
    assert!(canvas.zoom_at(100.0, 100.0, 40.0));
    near(canvas.zoom(), 8.0);
    assert!(!canvas.zoom_at(0.0, 0.0, 0.0));
    assert!(!canvas.zoom_at(f64::NAN, 0.0, 0.0));
    near(canvas.zoom(), 8.0);
    assert!(canvas.pan_by(10.0, -4.0));
    near(canvas.pan().0, 100.0 - 100.0 * 8.0 + 10.0);
    assert!(!canvas.pan_by(f64::INFINITY, 0.0));
}

#[test]
fn a_point_hits_the_frontmost_frame_and_misses_the_gap() {
    let mut canvas = Canvas::new();
    canvas.open_frame(LOGIN.as_ref()).unwrap();
    canvas.open_frame(MENU.as_ref()).unwrap();
    assert_eq!(canvas.frame_at(10.0, 10.0), Some(0));
    assert_eq!(canvas.frame_at(400.0, 10.0), None);
    assert_eq!(canvas.frame_at(470.0, 0.0), Some(1));
    assert_eq!(canvas.frame_at(470.0 + 390.0, 0.0), None);
    assert!(canvas.move_frame(1, 100.0, 0.0));
    assert_eq!(canvas.frame_at(150.0, 20.0), Some(1));
    assert!(canvas.pan_by(20.0, 0.0));
    assert_eq!(canvas.frame_at(30.0, 10.0), Some(0));
    assert_eq!(canvas.frame_at(f64::NAN, 0.0), None);
    assert!(!canvas.move_frame(1, f64::NAN, 0.0));
    assert!(!canvas.move_frame(4, 0.0, 0.0));
}

#[test]
fn focus_walks_the_element_tree_and_steps_over_a_slot() {
    let mut canvas = Canvas::new();
    canvas.open_frame(LOGIN.as_ref()).unwrap();
    let paths = canvas.elements(0).unwrap();
    assert_eq!(paths, login_paths());
    assert!(!canvas.select(0, "/screen#login/nope"));
    assert!(canvas.selection().is_none());
    assert!(canvas.focus(Focus::Next));
    assert_eq!(canvas.selection(), Some((0, "/screen#login")));
    assert!(canvas.focus(Focus::Child));
    assert_eq!(canvas.selection(), Some((0, "/screen#login/form#form")));
    assert!(canvas.focus(Focus::Child));
    assert_eq!(
        canvas.selection(),
        Some((0, "/screen#login/form#form/heading#title"))
    );
    assert!(!canvas.focus(Focus::Child));
    assert!(canvas.select(0, "/screen#login/form#form/button#submit"));
    assert!(canvas.focus(Focus::Next));
    assert_eq!(
        canvas.selection(),
        Some((0, "/screen#login/form#form/slot[footer]/link#reset"))
    );
    assert!(canvas.focus(Focus::Parent));
    assert_eq!(canvas.selection(), Some((0, "/screen#login/form#form")));
    assert!(canvas.select(0, "/screen#login"));
    assert!(!canvas.focus(Focus::Parent));
    assert!(canvas.select(0, paths.last().unwrap()));
    assert!(!canvas.focus(Focus::Next));
    assert!(canvas.focus(Focus::Previous));
    assert_eq!(
        canvas.selection(),
        Some((0, "/screen#login/form#form/slot[footer]/link#reset"))
    );
}

#[test]
fn focus_crosses_frames_and_hover_names_an_element() {
    let mut canvas = Canvas::new();
    canvas.open_frame(LOGIN.as_ref()).unwrap();
    canvas.open_frame(MENU.as_ref()).unwrap();
    let last_login = canvas.elements(0).unwrap().pop().unwrap();
    assert!(canvas.select(0, &last_login));
    assert!(canvas.focus(Focus::Next));
    assert_eq!(canvas.selection().unwrap().0, 1);
    assert!(canvas.selection().unwrap().1.starts_with("/screen#"));
    let mut bare = Canvas::new();
    assert!(!bare.focus(Focus::Next));
    assert!(!canvas.hover_at(1, Some("/nope")));
    let menu_root = canvas.elements(1).unwrap().into_iter().next().unwrap();
    assert!(canvas.hover_at(1, Some(&menu_root)));
    assert_eq!(canvas.hover(), Some((1, menu_root.as_str())));
    assert!(canvas.hover_at(1, None));
    assert!(canvas.hover().is_none());
}
