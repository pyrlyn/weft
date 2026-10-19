//! The canvas through the C ABI a shell calls, headless.
//!
//! Slint takes the first thread that draws as its UI thread for the whole process, so only one
//! test here makes a surface. `tests/e2e.rs` draws the single-screen editor in its own binary.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]
// It drives the C ABI, as a shell does.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::{Path, PathBuf};
use std::ptr;

use serde_json::{Value as Json, json};
use slint_bindings_ffi::{sb_host_frame_len, sb_host_free, sb_host_render};
use weft_studio_ffi::WsCanvas;
use weft_studio_ffi::abi::{
    ws_canvas_free, ws_canvas_hover, ws_canvas_move, ws_canvas_open_dir, ws_canvas_save,
    ws_canvas_select, ws_canvas_set_view, ws_canvas_state_json, ws_canvas_surface_new,
    ws_last_error, ws_string_free,
};
use weft_studio_ffi::{
    WS_CANVAS_NEXT, WS_CANVAS_NEXT_FRAME, WS_CANVAS_PARENT, WS_CANVAS_PREVIOUS_FRAME,
};

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("weft-studio-canvas-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, content: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn copy_screen(&self, corpus: &str, name: &str) -> PathBuf {
        let bytes = std::fs::read(Path::new(CORPUS).join(corpus).join("screen.weft")).unwrap();
        self.write(name, &bytes)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn c(path: &Path) -> CString {
    CString::new(path.to_str().unwrap()).unwrap()
}

fn last_error() -> String {
    let text = ws_last_error();
    assert!(!text.is_null(), "a failed call left no message");
    // SAFETY: a non-null `ws_last_error` is NUL-terminated until the next call.
    unsafe { CStr::from_ptr(text) }.to_str().unwrap().to_owned()
}

struct Slot {
    canvas: *mut WsCanvas,
    events: Vec<Json>,
    reread: bool,
}

extern "C" fn record(user_data: *mut c_void, event: *const c_char) {
    // SAFETY: `user_data` is the test's `Slot`, live while its canvas and surface are.
    let slot = unsafe { &mut *user_data.cast::<Slot>() };
    // SAFETY: the event is NUL-terminated for the length of the call.
    let text = unsafe { CStr::from_ptr(event) }.to_str().unwrap();
    slot.events.push(serde_json::from_str(text).unwrap());
    if slot.canvas.is_null() {
        return;
    }
    // SAFETY: a live canvas. The call must not still be borrowing it.
    let state = unsafe { ws_canvas_state_json(slot.canvas) };
    assert!(!state.is_null(), "changed could not read the state");
    // SAFETY: a string from `ws_canvas_state_json`, freed once.
    unsafe { ws_string_free(state) };
    slot.reread = true;
}

fn open(dir: &Path, slot: &mut Slot) -> *mut WsCanvas {
    // SAFETY: a NUL-terminated path. Opening does not call back, so `slot.canvas` may be null.
    let canvas =
        unsafe { ws_canvas_open_dir(c(dir).as_ptr(), Some(record), ptr::from_mut(slot).cast()) };
    assert!(!canvas.is_null(), "{}", last_error());
    slot.canvas = canvas;
    canvas
}

fn state_of(canvas: *mut WsCanvas) -> Json {
    // SAFETY: a live canvas.
    let text = unsafe { ws_canvas_state_json(canvas) };
    assert!(!text.is_null(), "{}", last_error());
    // SAFETY: a string from `ws_canvas_state_json`, freed once below.
    let json = serde_json::from_str(unsafe { CStr::from_ptr(text) }.to_str().unwrap()).unwrap();
    // SAFETY: as above.
    unsafe { ws_string_free(text) };
    json
}

#[test]
fn a_directory_lists_both_screens_and_the_selection_moves() {
    let scratch = Scratch::new("pair");
    scratch.copy_screen("login", "login.weft");
    scratch.copy_screen("account", "account.weft");
    let mut slot = Slot {
        canvas: ptr::null_mut(),
        events: Vec::new(),
        reread: false,
    };
    let canvas = open(&scratch.0, &mut slot);
    assert!(slot.events.is_empty(), "open sends no event");
    let state = state_of(canvas);
    assert_eq!(state["v"], 0);
    assert_eq!(state["frames"][0]["title"], "account.weft");
    assert_eq!(state["frames"][1]["title"], "login.weft");
    assert_eq!(state["frames"][0]["screen"], "account");
    assert_eq!(state["frames"][1]["screen"], "login");
    assert_eq!(state["selection"], Json::Null);
    assert_eq!(state["zoom"], 1.0);

    // SAFETY: a live canvas.
    unsafe {
        assert!(ws_canvas_set_view(canvas, -80.0, f64::NAN, 9.0));
        assert!(ws_canvas_move(canvas, WS_CANVAS_NEXT));
        assert!(ws_canvas_move(canvas, WS_CANVAS_PARENT));
        assert!(ws_canvas_move(canvas, WS_CANVAS_NEXT_FRAME));
        assert!(ws_canvas_move(canvas, WS_CANVAS_PREVIOUS_FRAME));
    }
    assert!(slot.reread, "changed did not re-enter");
    assert!(
        slot.events
            .iter()
            .all(|event| *event == json!({ "v": 0, "type": "changed" }))
    );
    let moved = state_of(canvas);
    assert_eq!(moved["pan_x"], json!(-80.0));
    assert_eq!(moved["pan_y"], json!(0.0));
    assert_eq!(moved["zoom"], json!(4.0));
    assert_eq!(moved["selection"]["frame"], 0);
    assert_eq!(moved["selection"]["path"], "/screen#account");
    // SAFETY: freed once.
    unsafe { ws_canvas_free(canvas) };
}

#[test]
fn a_save_round_trips_and_a_null_path_writes_the_current_file() {
    let scratch = Scratch::new("save");
    let account = scratch.copy_screen("account", "account.weft");
    scratch.copy_screen("login", "login.weft");
    let original = std::fs::read(&account).unwrap();
    let copy = scratch.0.join("copy.weft");
    let mut slot = Slot {
        canvas: ptr::null_mut(),
        events: Vec::new(),
        reread: false,
    };
    let canvas = open(&scratch.0, &mut slot);
    // SAFETY: a live canvas. A null path saves the frame in place. Frame 0 is account.weft.
    unsafe {
        assert!(ws_canvas_save(canvas, 0, ptr::null()), "{}", last_error());
        assert_eq!(std::fs::read(&account).unwrap(), original);
        assert!(
            ws_canvas_save(canvas, 0, c(&copy).as_ptr()),
            "{}",
            last_error()
        );
        assert_eq!(std::fs::read(&copy).unwrap(), original);
    }
    assert_eq!(
        state_of(canvas)["frames"][0]["path"],
        copy.to_str().unwrap()
    );
    assert_eq!(state_of(canvas)["frames"][0]["title"], "copy.weft");
    assert_eq!(slot.events.len(), 2);
    // SAFETY: freed once.
    unsafe { ws_canvas_free(canvas) };
}

#[test]
fn bad_input_fails_and_a_menu_stays_out_of_the_window() {
    let scratch = Scratch::new("errors");
    let empty = Scratch::new("empty");
    empty.write("notes.txt", b"no");
    let syntax = Scratch::new("syntax");
    syntax.write("broken.weft", b"<screen id=\"s\" weft=\"0.1\"><heading");
    scratch.copy_screen("login", "login.weft");
    scratch.copy_screen("menu", "menu.weft");
    let mut slot = Slot {
        canvas: ptr::null_mut(),
        events: Vec::new(),
        reread: false,
    };
    // SAFETY: each pointer is null, a live canvas or a NUL-terminated string.
    unsafe {
        assert!(
            ws_canvas_open_dir(ptr::null(), Some(record), ptr::from_mut(&mut slot).cast())
                .is_null()
        );
        assert!(last_error().contains("null path"), "{}", last_error());
        assert!(ws_canvas_open_dir(c(&empty.0).as_ptr(), None, ptr::null_mut()).is_null());
        assert!(last_error().contains("no .weft file"), "{}", last_error());
        assert!(ws_canvas_open_dir(c(&syntax.0).as_ptr(), None, ptr::null_mut()).is_null());
        assert!(last_error().contains("syntax"), "{}", last_error());

        assert!(ws_canvas_state_json(ptr::null()).is_null());
        assert!(last_error().contains("null canvas"), "{}", last_error());
        assert!(!ws_canvas_set_view(ptr::null_mut(), 0.0, 0.0, 1.0));
        assert!(!ws_canvas_move(ptr::null_mut(), WS_CANVAS_NEXT));
        assert!(!ws_canvas_save(ptr::null_mut(), 0, ptr::null()));
        assert!(!ws_canvas_select(ptr::null_mut(), 0, ptr::null()));
        assert!(!ws_canvas_hover(ptr::null_mut(), 0, ptr::null()));
        assert!(ws_canvas_surface_new(ptr::null_mut(), 320, 240, 1.0).is_null());

        let canvas = open(&scratch.0, &mut slot);
        assert!(!ws_canvas_move(canvas, 9));
        assert!(
            last_error().contains("unknown canvas direction"),
            "{}",
            last_error()
        );
        assert!(!ws_canvas_save(canvas, 9, ptr::null()));
        assert!(last_error().contains("no frame"), "{}", last_error());
        let state = state_of(canvas);
        assert_eq!(state["frames"][0]["embedded"], true);
        assert_eq!(state["frames"][1]["embedded"], false);
        assert!(
            state["frames"][1]["error"]
                .as_str()
                .unwrap()
                .contains("menu"),
            "{}",
            state["frames"][1]["error"]
        );
        ws_canvas_free(canvas);
        ws_canvas_free(ptr::null_mut());
    }
    assert!(slot.events.is_empty(), "a failed call is not an event");
}

#[test]
fn selection_and_hover_name_an_element_or_clear() {
    let scratch = Scratch::new("aim");
    scratch.copy_screen("login", "login.weft");
    scratch.copy_screen("account", "account.weft");
    let mut slot = Slot {
        canvas: ptr::null_mut(),
        events: Vec::new(),
        reread: false,
    };
    let canvas = open(&scratch.0, &mut slot);
    let account = CString::new("/screen#account").unwrap();
    let login = CString::new("/screen#login").unwrap();
    let missing = CString::new("/screen#missing").unwrap();
    let not_utf8 = CString::new(vec![b'/', 0xff]).unwrap();
    // SAFETY: a live canvas and NUL-terminated strings. A null path clears.
    unsafe {
        assert!(
            ws_canvas_select(canvas, 0, account.as_ptr()),
            "{}",
            last_error()
        );
        assert!(!ws_canvas_select(canvas, 0, missing.as_ptr()));
        assert!(last_error().contains("no element"), "{}", last_error());
        assert!(!ws_canvas_select(canvas, 0, not_utf8.as_ptr()));
        assert!(last_error().contains("UTF-8"), "{}", last_error());
        assert!(
            ws_canvas_hover(canvas, 1, login.as_ptr()),
            "{}",
            last_error()
        );
    }
    let aimed = state_of(canvas);
    assert_eq!(aimed["selection"]["frame"], 0);
    assert_eq!(aimed["selection"]["path"], "/screen#account");
    assert_eq!(aimed["hover"]["frame"], 1);
    assert_eq!(aimed["hover"]["path"], "/screen#login");
    // SAFETY: a live canvas. Null clears.
    unsafe {
        assert!(ws_canvas_select(canvas, 0, ptr::null()));
        assert!(ws_canvas_hover(canvas, 9, ptr::null()));
    }
    let cleared = state_of(canvas);
    assert_eq!(cleared["selection"], Json::Null);
    assert_eq!(cleared["hover"], Json::Null);
    assert!(slot.reread, "changed did not re-enter");
    assert_eq!(slot.events.len(), 4);
    // SAFETY: freed once.
    unsafe { ws_canvas_free(canvas) };
}

#[test]
fn a_call_from_another_thread_fails() {
    let scratch = Scratch::new("thread");
    scratch.copy_screen("login", "login.weft");
    // SAFETY: a NUL-terminated path.
    let canvas =
        unsafe { ws_canvas_open_dir(c(&scratch.0).as_ptr(), None, ptr::null_mut()) } as usize;
    assert_ne!(canvas, 0, "{}", last_error());
    let message = std::thread::spawn(move || {
        // SAFETY: a live canvas, used from the wrong thread on purpose.
        assert!(!unsafe { ws_canvas_set_view(canvas as *mut WsCanvas, 1.0, 2.0, 1.0) });
        last_error()
    })
    .join()
    .unwrap();
    assert!(message.contains("thread"), "{message}");
    // SAFETY: freed once, on its own thread.
    unsafe { ws_canvas_free(canvas as *mut WsCanvas) };
}

#[test]
fn the_canvas_surface_paints_something() {
    let scratch = Scratch::new("paint");
    scratch.copy_screen("login", "login.weft");
    scratch.copy_screen("account", "account.weft");
    let mut slot = Slot {
        canvas: ptr::null_mut(),
        events: Vec::new(),
        reread: false,
    };
    let canvas = open(&scratch.0, &mut slot);
    // SAFETY: a live canvas.
    let host = unsafe { ws_canvas_surface_new(canvas, 320, 240, 1.0) };
    assert!(!host.is_null(), "{}", last_error());
    // SAFETY: a live host and a buffer of the length it asks for.
    let len = unsafe { sb_host_frame_len(host) };
    assert_eq!(len, 320 * 240 * 4);
    let mut pixels = vec![0u8; len];
    // SAFETY: as above.
    let frame = unsafe { sb_host_render(host, pixels.as_mut_ptr(), len) };
    assert_eq!(frame.failed, 0);
    assert!(
        pixels.chunks(4).any(|pixel| pixel[3] != 0),
        "the frame is transparent"
    );
    // SAFETY: each handle is live and freed once.
    unsafe {
        sb_host_free(host);
        ws_canvas_free(canvas);
    }
}
