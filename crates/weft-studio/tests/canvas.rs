//! Several screens as frames: order, pan and zoom, the element tree, and one canvas window.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use weft_core::{ParseOptions, element_paths, parse};
use weft_slint::runtime;
use weft_studio::{Canvas, CanvasError, Direction, FRAME_GAP, FRAME_WIDTH};

const LOGIN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/login/screen.weft"
);
const ACCOUNT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../corpus/account/screen.weft"
);
const MENU: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/menu/screen.weft");

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("weft-canvas-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pair() -> (Scratch, Canvas) {
    let scratch = Scratch::new("pair");
    let login = scratch.write("login.weft", &std::fs::read_to_string(LOGIN).unwrap());
    let account = scratch.write("account.weft", &std::fs::read_to_string(ACCOUNT).unwrap());
    let canvas = Canvas::open_files(&[login, account]).unwrap();
    (scratch, canvas)
}

#[test]
fn files_keep_the_given_order_and_a_directory_sorts_them() {
    let (scratch, canvas) = pair();
    let state = canvas.state();
    assert_eq!(state.frames.len(), 2);
    assert_eq!(state.frames[0].title, "login.weft");
    assert_eq!(state.frames[1].title, "account.weft");
    assert_eq!(state.frames[0].screen.as_deref(), Some("login"));
    assert_eq!(state.frames[0].x, 0.0);
    assert_eq!(state.frames[1].x, FRAME_WIDTH + FRAME_GAP);
    assert!(state.frames.iter().all(|frame| frame.embedded), "{state:?}");
    assert_eq!(state.zoom, 1.0);

    let opened = Canvas::open_dir(&scratch.0).unwrap();
    let titles: Vec<_> = opened
        .state()
        .frames
        .iter()
        .map(|frame| frame.title.clone())
        .collect();
    assert_eq!(titles, ["account.weft", "login.weft"]);
}

#[test]
fn an_empty_directory_and_an_empty_list_are_refused() {
    let scratch = Scratch::new("empty");
    scratch.write("notes.txt", "no");
    assert!(matches!(
        Canvas::open_dir(&scratch.0),
        Err(CanvasError::Empty(_))
    ));
    let none: [&str; 0] = [];
    assert!(matches!(
        Canvas::open_files(&none),
        Err(CanvasError::NoScreens)
    ));
}

#[test]
fn pan_and_zoom_are_clamped_and_written_into_the_window() {
    let (_scratch, mut canvas) = pair();
    canvas.set_pan(-80.0, 12.5);
    canvas.set_pan(f64::NAN, 4.0);
    canvas.set_zoom(10.0);
    canvas.set_zoom(f64::NAN);
    let state = canvas.state();
    assert_eq!(state.pan_x, -80.0);
    assert_eq!(state.pan_y, 4.0);
    assert_eq!(state.zoom, 4.0);
    canvas.set_zoom(0.1);
    assert_eq!(canvas.state().zoom, 0.25);

    let source = canvas.render_input().unwrap().source;
    assert!(source.contains("in-out property <length> pan-x: -80px;"));
    assert!(source.contains("in-out property <length> pan-y: 4px;"));
    assert!(source.contains("in-out property <float> zoom: 0.25;"));
    assert!(source.contains("component LoginScreen {"));
    assert!(source.contains("component AccountScreen {"));
    assert!(source.contains("for tick in 24: Rectangle {"));
    assert_eq!(source.matches("export component ").count(), 1);
    let compiled =
        runtime::compile(&source, "fluent").unwrap_or_else(|errors| panic!("{errors:?}\n{source}"));
    assert_eq!(compiled.name(), "WeftStudioCanvas");
}

#[test]
fn selection_walks_the_element_tree_and_the_frames() {
    let (_scratch, mut canvas) = pair();
    let paths = canvas.paths(0).unwrap();
    let parsed = parse(
        &std::fs::read_to_string(LOGIN).unwrap(),
        &ParseOptions::default(),
    )
    .document
    .unwrap();
    let mut from_core: Vec<_> = element_paths(&parsed)
        .into_iter()
        .map(|(_, path)| path)
        .collect();
    let mut ordered = paths.clone();
    from_core.sort();
    ordered.sort();
    assert_eq!(ordered, from_core);
    assert_eq!(paths[0], "/screen#login");
    assert_eq!(paths[1], "/screen#login/form#form");

    assert_eq!(canvas.move_selection(Direction::Previous), None);
    assert_eq!(
        canvas.move_selection(Direction::Next).unwrap(),
        "/screen#login"
    );
    assert_eq!(
        canvas.move_selection(Direction::Next).unwrap(),
        "/screen#login/form#form"
    );
    let button = "/screen#login/form#form/button#submit";
    canvas.select(0, button).unwrap();
    assert_eq!(
        canvas.move_selection(Direction::Parent).unwrap(),
        "/screen#login/form#form"
    );
    assert_eq!(
        canvas.move_selection(Direction::Parent).unwrap(),
        "/screen#login"
    );
    // The screen has no parent, so the selection stays.
    assert_eq!(
        canvas.move_selection(Direction::Parent).unwrap(),
        "/screen#login"
    );
    canvas
        .select(0, "/screen#login/form#form/slot[footer]/link#reset")
        .unwrap();
    assert_eq!(
        canvas.move_selection(Direction::Parent).unwrap(),
        "/screen#login/form#form"
    );

    let account_root = canvas.paths(1).unwrap()[0].clone();
    assert_eq!(
        canvas.move_selection(Direction::NextFrame).unwrap(),
        account_root
    );
    assert_eq!(
        canvas.move_selection(Direction::NextFrame).unwrap(),
        "/screen#login"
    );
    assert!(matches!(
        canvas.select(0, "/screen#login/missing"),
        Err(CanvasError::Element { .. })
    ));
    assert!(matches!(canvas.paths(4), Err(CanvasError::Frame(4))));
    canvas.set_hover(1, &canvas.paths(1).unwrap()[0]).unwrap();
    let state = canvas.state();
    assert_eq!(state.hover.unwrap().frame, 1);
    assert_eq!(state.selection.unwrap().path, "/screen#login");
    let source = canvas.render_input().unwrap().source;
    assert!(source.contains("selected-frame: 0;"));
    assert!(source.contains("selected-path: \"/screen#login\";"));
}

#[test]
fn a_menu_stays_on_the_canvas_and_out_of_the_window() {
    let scratch = Scratch::new("menu");
    let login = scratch.write("login.weft", &std::fs::read_to_string(LOGIN).unwrap());
    let menu = scratch.write("menu.weft", &std::fs::read_to_string(MENU).unwrap());
    let mut canvas = Canvas::open_files(&[login, menu.clone()]).unwrap();
    let state = canvas.state();
    assert!(state.frames[0].embedded);
    assert!(!state.frames[1].embedded);
    assert!(state.frames[1].error.as_deref().unwrap().contains("menu"));
    // The menu is still a document: its elements can be selected, and it saves.
    let root = canvas.paths(1).unwrap()[0].clone();
    canvas.select(1, &root).unwrap();
    let source = canvas.render_input().unwrap().source;
    assert!(source.contains("component LoginScreen {"));
    assert!(!source.contains("component MenuScreen {"));
    let copy = scratch.0.join("menu-copy.weft");
    canvas.save(1, Some(&copy)).unwrap();
    assert_eq!(std::fs::read(&copy).unwrap(), std::fs::read(&menu).unwrap());
}

#[test]
fn a_syntax_error_refuses_the_open() {
    let scratch = Scratch::new("syntax");
    let login = scratch.write("login.weft", &std::fs::read_to_string(LOGIN).unwrap());
    let broken = scratch.write("broken.weft", "<screen id=\"s\" weft=\"0.1\"><heading");
    assert!(matches!(
        Canvas::open_files(&[login, broken]),
        Err(CanvasError::Editor(_))
    ));
}

#[test]
fn two_stems_that_name_one_component_are_split() {
    let scratch = Scratch::new("clash");
    let first = scratch.write("foo-bar.weft", &std::fs::read_to_string(LOGIN).unwrap());
    let second = scratch.write("foo_bar.weft", &std::fs::read_to_string(ACCOUNT).unwrap());
    let canvas = Canvas::open_files(&[first, second]).unwrap();
    let source = canvas.render_input().unwrap().source;
    assert_eq!(source.matches("component ").count(), 3);
    assert!(runtime::compile(&source, "fluent").is_ok());
}
