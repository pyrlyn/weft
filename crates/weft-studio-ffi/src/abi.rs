//! The `ws_*` entry points. Each checks its pointers and strings, runs inside the guard, and sends
//! events only after it no longer borrows the editor or the canvas, because a shell's callback may
//! call back in (`ws_state_json` on `changed`). Opening a canvas does not send `changed`: the
//! caller does not hold the pointer until `ws_canvas_open_dir` returns.

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::Path;
use std::ptr;

use slint_bindings_ffi::SbHost;
use weft_studio::Direction;

use crate::{
    WS_API_VERSION, WS_CANVAS_NEXT, WS_CANVAS_NEXT_FRAME, WS_CANVAS_PARENT, WS_CANVAS_PREVIOUS,
    WS_CANVAS_PREVIOUS_FRAME, WsCanvas, WsEditor, WsEventFn, changed, guard, last_error,
};

/// # Safety
/// `text` is null or a NUL-terminated string valid for the call.
unsafe fn text_arg<'a>(text: *const c_char) -> Result<Option<&'a str>, String> {
    if text.is_null() {
        return Ok(None);
    }
    // SAFETY: forwarded caller contract.
    let text = unsafe { CStr::from_ptr(text) };
    text.to_str()
        .map(Some)
        .map_err(|_| "the element path is not valid UTF-8".into())
}

/// # Safety
/// `text` is null or a NUL-terminated string valid for the call.
unsafe fn path_arg<'a>(text: *const c_char) -> Result<&'a Path, String> {
    if text.is_null() {
        return Err("a null path was passed".into());
    }
    // SAFETY: forwarded caller contract.
    let text = unsafe { CStr::from_ptr(text) };
    text.to_str()
        .map(Path::new)
        .map_err(|_| "the path is not valid UTF-8".into())
}

/// # Safety
/// `editor` is null or a live editor from `ws_editor_new`.
unsafe fn editor_ref<'a>(editor: *const WsEditor) -> Result<&'a WsEditor, String> {
    // SAFETY: forwarded caller contract.
    let editor = unsafe { editor.as_ref() }.ok_or("a null editor was passed")?;
    editor.check_thread()?;
    Ok(editor)
}

/// # Safety
/// `canvas` is null or a live canvas from `ws_canvas_open_dir`.
unsafe fn canvas_ref<'a>(canvas: *const WsCanvas) -> Result<&'a WsCanvas, String> {
    // SAFETY: forwarded caller contract.
    let canvas = unsafe { canvas.as_ref() }.ok_or("a null canvas was passed")?;
    canvas.check_thread()?;
    Ok(canvas)
}

/// # Safety
/// `canvas` is null or a live canvas from `ws_canvas_open_dir`, not borrowed elsewhere.
unsafe fn canvas_mut<'a>(canvas: *mut WsCanvas) -> Result<&'a mut WsCanvas, String> {
    // SAFETY: forwarded caller contract.
    let canvas = unsafe { canvas.as_mut() }.ok_or("a null canvas was passed")?;
    canvas.check_thread()?;
    Ok(canvas)
}

fn canvas_direction(code: u32) -> Result<Direction, String> {
    match code {
        WS_CANVAS_NEXT => Ok(Direction::Next),
        WS_CANVAS_PREVIOUS => Ok(Direction::Previous),
        WS_CANVAS_PARENT => Ok(Direction::Parent),
        WS_CANVAS_NEXT_FRAME => Ok(Direction::NextFrame),
        WS_CANVAS_PREVIOUS_FRAME => Ok(Direction::PreviousFrame),
        other => Err(format!("unknown canvas direction {other}")),
    }
}

/// # Safety
/// `editor` is null or a live editor from `ws_editor_new`, not borrowed elsewhere.
unsafe fn editor_mut<'a>(editor: *mut WsEditor) -> Result<&'a mut WsEditor, String> {
    // SAFETY: forwarded caller contract.
    let editor = unsafe { editor.as_mut() }.ok_or("a null editor was passed")?;
    editor.check_thread()?;
    Ok(editor)
}

/// The API version, 0; a shell refuses any other.
#[unsafe(no_mangle)]
pub extern "C" fn ws_api_version() -> u32 {
    WS_API_VERSION
}

/// A new editor with no document. `on_event` (may be null) receives `user_data` and each event
/// as JSON. Free with `ws_editor_free`.
#[unsafe(no_mangle)]
pub extern "C" fn ws_editor_new(on_event: WsEventFn, user_data: *mut c_void) -> *mut WsEditor {
    guard(ptr::null_mut(), || {
        Ok(Box::into_raw(Box::new(WsEditor::new(on_event, user_data))))
    })
}

/// Frees an editor; null is ignored. Surfaces made from it stay valid.
///
/// # Safety
/// `editor` is null or a live editor from `ws_editor_new`, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_editor_free(editor: *mut WsEditor) {
    guard((), || {
        if !editor.is_null() {
            // SAFETY: the caller hands over the editor `ws_editor_new` boxed.
            drop(unsafe { Box::from_raw(editor) });
        }
        Ok(())
    });
}

/// Opens the `.weft` markup at the UTF-8 `path` with the project above it. False, with a message
/// and the syntax diagnostics in `ws_last_error`, when it cannot; the open document stays.
///
/// # Safety
/// `editor` is null or live; `path` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_open(editor: *mut WsEditor, path: *const c_char) -> bool {
    guard(false, || {
        // SAFETY: forwarded caller contract.
        let (editor, path) = unsafe { (editor_mut(editor)?, path_arg(path)?) };
        editor.open(path)?;
        let sink = editor.sink;
        sink.emit(&changed());
        Ok(true)
    })
}

/// Saves the canonical markup atomically to the UTF-8 `path`, or to the current path when `path`
/// is null; another path becomes the document's path.
///
/// # Safety
/// `editor` is null or live; `path` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_save(editor: *mut WsEditor, path: *const c_char) -> bool {
    guard(false, || {
        // SAFETY: forwarded caller contract.
        let editor = unsafe { editor_mut(editor)? };
        let path = if path.is_null() {
            None
        } else {
            // SAFETY: forwarded caller contract.
            Some(unsafe { path_arg(path)? })
        };
        editor.save(path)?;
        let sink = editor.sink;
        sink.emit(&changed());
        Ok(true)
    })
}

/// The open document's state as JSON (`v`, `path`, `title`, `screen`, `dirty`, `diagnostics`,
/// `project`, `project_diagnostics`). Null without a document. Free with `ws_string_free`.
///
/// # Safety
/// `editor` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_state_json(editor: *const WsEditor) -> *mut c_char {
    guard(ptr::null_mut(), || {
        // SAFETY: forwarded caller contract.
        let json = unsafe { editor_ref(editor)? }.state_json()?;
        CString::new(json)
            .map(CString::into_raw)
            .map_err(|e| e.to_string())
    })
}

/// A new surface showing the open screen at `width`×`height` physical pixels and `scale`, drawn
/// on the CPU. The shell owns it: draw and feed it with the `sb_host_*` calls, free it with
/// `sb_host_free`. Null, with the generator's or the compiler's message, when the screen is
/// refused.
///
/// # Safety
/// `editor` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_surface_new(
    editor: *mut WsEditor,
    width: u32,
    height: u32,
    scale: f32,
) -> *mut SbHost {
    guard(ptr::null_mut(), || {
        // SAFETY: forwarded caller contract.
        let editor = unsafe { editor_ref(editor)? };
        Ok(editor.surface(width, height, scale)?.into_raw())
    })
}

/// Opens every `.weft` file directly in the UTF-8 directory `path`, in file-name order, as one
/// canvas. Null, with a message in `ws_last_error`, when the directory cannot be read, holds no
/// `.weft` file, or a file has a syntax error. No `changed` event. Free with `ws_canvas_free`.
///
/// # Safety
/// `path` is null or a NUL-terminated string. `on_event` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_canvas_open_dir(
    path: *const c_char,
    on_event: WsEventFn,
    user_data: *mut c_void,
) -> *mut WsCanvas {
    guard(ptr::null_mut(), || {
        // SAFETY: forwarded caller contract.
        let path = unsafe { path_arg(path)? };
        Ok(Box::into_raw(Box::new(WsCanvas::open_dir(
            path, on_event, user_data,
        )?)))
    })
}

/// Frees a canvas; null is ignored. Surfaces made from it stay valid.
///
/// # Safety
/// `canvas` is null or a live canvas from `ws_canvas_open_dir`, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_canvas_free(canvas: *mut WsCanvas) {
    guard((), || {
        if !canvas.is_null() {
            // SAFETY: the caller hands over the canvas `ws_canvas_open_dir` boxed.
            drop(unsafe { Box::from_raw(canvas) });
        }
        Ok(())
    });
}

/// The canvas state as JSON (`v`, `frames`, `pan_x`, `pan_y`, `zoom`, `selection`, `hover`).
/// Free with `ws_string_free`.
///
/// # Safety
/// `canvas` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_canvas_state_json(canvas: *const WsCanvas) -> *mut c_char {
    guard(ptr::null_mut(), || {
        // SAFETY: forwarded caller contract.
        let json = unsafe { canvas_ref(canvas)? }.state_json()?;
        CString::new(json)
            .map(CString::into_raw)
            .map_err(|e| e.to_string())
    })
}

/// Sets pan and zoom. A non-finite pan leaves that axis; a non-finite zoom leaves the zoom; zoom
/// clamps to 1/4 through 4. A `changed` event follows.
///
/// # Safety
/// `canvas` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_canvas_set_view(
    canvas: *mut WsCanvas,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
) -> bool {
    guard(false, || {
        // SAFETY: forwarded caller contract.
        let canvas = unsafe { canvas_mut(canvas)? };
        canvas.set_view(pan_x, pan_y, zoom);
        let sink = canvas.sink;
        sink.emit(&changed());
        Ok(true)
    })
}

/// Rounds pan to the nearest multiple of the 48px frame gap. Halfway rounds away from zero.
/// A `changed` event follows.
///
/// # Safety
/// `canvas` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_canvas_snap(canvas: *mut WsCanvas) -> bool {
    guard(false, || {
        // SAFETY: forwarded caller contract.
        let canvas = unsafe { canvas_mut(canvas)? };
        canvas.snap_pan();
        let sink = canvas.sink;
        sink.emit(&changed());
        Ok(true)
    })
}

/// Moves the selection. `direction` is `WS_CANVAS_NEXT` (0), `WS_CANVAS_PREVIOUS` (1),
/// `WS_CANVAS_PARENT` (2), `WS_CANVAS_NEXT_FRAME` (3) or `WS_CANVAS_PREVIOUS_FRAME` (4). Anything
/// else fails and leaves the selection. A `changed` event follows a move.
///
/// # Safety
/// `canvas` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_canvas_move(canvas: *mut WsCanvas, direction: u32) -> bool {
    guard(false, || {
        // SAFETY: forwarded caller contract.
        let canvas = unsafe { canvas_mut(canvas)? };
        let direction = canvas_direction(direction)?;
        canvas.move_selection(direction);
        let sink = canvas.sink;
        sink.emit(&changed());
        Ok(true)
    })
}

/// Selects the element `path` on frame `frame` (0-based). A null `path` clears the selection.
/// An unknown element fails and leaves the selection. A `changed` event follows.
///
/// # Safety
/// `canvas` is null or live; `path` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_canvas_select(
    canvas: *mut WsCanvas,
    frame: u32,
    path: *const c_char,
) -> bool {
    guard(false, || {
        // SAFETY: forwarded caller contract.
        let canvas = unsafe { canvas_mut(canvas)? };
        let path = unsafe { text_arg(path)? };
        canvas.select(frame, path)?;
        let sink = canvas.sink;
        sink.emit(&changed());
        Ok(true)
    })
}

/// Hovers the element `path` on frame `frame` (0-based). A null `path` clears the hover. An
/// unknown element fails and leaves the hover. A `changed` event follows.
///
/// # Safety
/// `canvas` is null or live; `path` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_canvas_hover(
    canvas: *mut WsCanvas,
    frame: u32,
    path: *const c_char,
) -> bool {
    guard(false, || {
        // SAFETY: forwarded caller contract.
        let canvas = unsafe { canvas_mut(canvas)? };
        let path = unsafe { text_arg(path)? };
        canvas.hover(frame, path)?;
        let sink = canvas.sink;
        sink.emit(&changed());
        Ok(true)
    })
}

/// Saves frame `frame` (0-based, in the order `ws_canvas_open_dir` opened) as canonical markup
/// to the UTF-8 `path`, or to its current path when `path` is null. A `changed` event follows.
///
/// # Safety
/// `canvas` is null or live; `path` is null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_canvas_save(
    canvas: *mut WsCanvas,
    frame: u32,
    path: *const c_char,
) -> bool {
    guard(false, || {
        // SAFETY: forwarded caller contract.
        let canvas = unsafe { canvas_mut(canvas)? };
        let path = if path.is_null() {
            None
        } else {
            // SAFETY: forwarded caller contract.
            Some(unsafe { path_arg(path)? })
        };
        canvas.save(frame, path)?;
        let sink = canvas.sink;
        sink.emit(&changed());
        Ok(true)
    })
}

/// A new surface showing the canvas at `width`×`height` physical pixels and `scale`, drawn on
/// the CPU. The shell owns it: draw and feed it with the `sb_host_*` calls, free it with
/// `sb_host_free`. Null, with the generator's or the compiler's message, when nothing can be
/// drawn. `perform` arrives as an `action` event with `screen`, `action` and `id`.
///
/// # Safety
/// `canvas` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_canvas_surface_new(
    canvas: *mut WsCanvas,
    width: u32,
    height: u32,
    scale: f32,
) -> *mut SbHost {
    guard(ptr::null_mut(), || {
        // SAFETY: forwarded caller contract.
        let canvas = unsafe { canvas_ref(canvas)? };
        Ok(canvas.surface(width, height, scale)?.into_raw())
    })
}

/// Frees a string from `ws_state_json` or `ws_canvas_state_json`; null is ignored.
///
/// # Safety
/// `text` is null or a string from a `ws_*` call, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ws_string_free(text: *mut c_char) {
    guard((), || {
        if !text.is_null() {
            // SAFETY: the caller hands back a string `CString::into_raw` made.
            drop(unsafe { CString::from_raw(text) });
        }
        Ok(())
    });
}

/// The message of this thread's last failed `ws_*` call, or null. Valid until the next call.
#[unsafe(no_mangle)]
pub extern "C" fn ws_last_error() -> *const c_char {
    last_error()
}
