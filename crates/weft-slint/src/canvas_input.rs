//! Rulers and pointer input for a [`compose`](crate::compose) canvas (T69 milestone 2).
//! The frames stay where `compose` put them. This module only adds the chrome that drives
//! `pan-x`, `pan-y` and `zoom`.

/// `source` is not the `Canvas` window [`crate::compose`] prints.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CanvasChromeError {
    #[error("the source is not a Canvas window from compose")]
    NotCanvas,
}

const EXPORT: &str = "export component Canvas inherits Window {\n";
const ZOOM: &str = "    in-out property <float> zoom: 1;\n";

/// Adds rulers, wheel zoom, middle-button pan and arrow-key pan to a [`crate::compose`] file.
///
/// Zoom stays between 0.25 and 4. Each ruler draws 40 ticks of 40px, following pan and zoom.
/// A left click still reaches the screen: only the middle button pans. A screen with a menu never
/// reaches here, because `compose` only accepts embedded components.
pub fn canvas_chrome(source: &str) -> Result<String, CanvasChromeError> {
    if source.matches(EXPORT).count() != 1 {
        return Err(CanvasChromeError::NotCanvas);
    }
    let Some((head, tail)) = source.split_once(ZOOM) else {
        return Err(CanvasChromeError::NotCanvas);
    };
    if !head.contains(EXPORT) {
        return Err(CanvasChromeError::NotCanvas);
    }
    let Some(close) = tail.rfind("}\n") else {
        return Err(CanvasChromeError::NotCanvas);
    };
    if !tail[close + 2..].trim().is_empty() {
        return Err(CanvasChromeError::NotCanvas);
    }
    let mut out = String::with_capacity(source.len() + 2048);
    out.push_str(head);
    out.push_str(ZOOM);
    out.push_str(&chrome(&tail[..close]));
    out.push_str("}\n");
    Ok(out)
}

fn chrome(frames: &str) -> String {
    let body = indent(frames);
    format!(
        "\
    forward-focus: canvas-keys;
    private property <bool> panning: false;
    private property <length> grab-x: 0px;
    private property <length> grab-y: 0px;
    private property <length> origin-x: 0px;
    private property <length> origin-y: 0px;

    canvas-keys := FocusScope {{
        key-pressed(event) => {{
            if (event.text == Key.LeftArrow) {{
                root.pan-x = root.pan-x + 40px;
                return accept;
            }}
            if (event.text == Key.RightArrow) {{
                root.pan-x = root.pan-x - 40px;
                return accept;
            }}
            if (event.text == Key.UpArrow) {{
                root.pan-y = root.pan-y + 40px;
                return accept;
            }}
            if (event.text == Key.DownArrow) {{
                root.pan-y = root.pan-y - 40px;
                return accept;
            }}
            return reject;
        }}
    }}

    canvas-input := TouchArea {{
        width: 100%;
        height: 100%;
        scroll-event(event) => {{
            if (event.delta-y > 0px) {{
                root.zoom = min(root.zoom * 1.1, 4);
            }} else if (event.delta-y < 0px) {{
                root.zoom = max(root.zoom / 1.1, 0.25);
            }}
            return accept;
        }}
        pointer-event(event) => {{
            if (event.button == PointerEventButton.middle && event.kind == PointerEventKind.down) {{
                root.panning = true;
                root.grab-x = self.mouse-x;
                root.grab-y = self.mouse-y;
                root.origin-x = root.pan-x;
                root.origin-y = root.pan-y;
            }}
            if (event.button == PointerEventButton.middle && (event.kind == PointerEventKind.up || event.kind == PointerEventKind.cancel)) {{
                root.panning = false;
            }}
        }}
        moved => {{
            if (root.panning) {{
                root.pan-x = root.origin-x + self.mouse-x - root.grab-x;
                root.pan-y = root.origin-y + self.mouse-y - root.grab-y;
            }}
        }}
{body}    }}

    ruler-x := Rectangle {{
        y: 0px;
        width: 100%;
        height: 24px;
        background: #f4f4f5;
        z: 1;
        for tick in 40: Rectangle {{
            x: root.pan-x + tick * 40px * root.zoom;
            y: 16px;
            width: 1px;
            height: 8px;
            background: #9a9aa3;
        }}
    }}
    ruler-y := Rectangle {{
        x: 0px;
        width: 24px;
        height: 100%;
        background: #f4f4f5;
        z: 1;
        for tick in 40: Rectangle {{
            y: root.pan-y + tick * 40px * root.zoom;
            x: 16px;
            width: 8px;
            height: 1px;
            background: #9a9aa3;
        }}
    }}
    ruler-corner := Rectangle {{
        width: 24px;
        height: 24px;
        background: #e6e6ea;
        z: 2;
    }}
"
    )
}

fn indent(block: &str) -> String {
    let mut out = String::new();
    for line in block.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}
