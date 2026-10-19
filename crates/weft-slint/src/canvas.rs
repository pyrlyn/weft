//! One Slint window that places several embedded screens on a canvas: pan, zoom, rulers and a grid.
//! The studio document core owns the frames; this module only prints the source. Zoom is a
//! `transform-scale` plus the frame boxes, because the language scale does not rewrite the px the
//! generator already emitted. Slint's software renderer does not apply transforms (Slint 1.18.1,
//! "Transforms"), so on that renderer the frames spread out and the screen inside stays at its
//! own size.

use std::collections::BTreeSet;

use crate::names::string;

/// One screen already printed by [`crate::generate_embedded`], and where the canvas puts it.
pub struct CanvasScreen {
    pub source: String,
    /// The screen's Weft id, passed to `perform` with the action and the element id.
    pub screen_id: String,
    /// The frame's index in the document, which `selected-frame` compares against.
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The view the window opens at. Lengths are logical pixels. `zoom` is a factor: 1 is the size
/// the generator wrote.
pub struct CanvasView {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
    pub selected_frame: Option<usize>,
    pub selected_path: String,
    pub hover_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AssembleError {
    #[error("the embedded source has no component")]
    NoComponent,
    #[error("a canvas length is not a finite number")]
    Number,
}

/// The canvas file: each screen as a private component, then one exported `WeftStudioCanvas` window.
/// `pan-x` and `pan-y` are the `Flickable`'s `content-x` and `content-y` (zero at the origin,
/// negative once the view has moved right or down).
pub fn assemble_canvas(
    screens: &[CanvasScreen],
    view: &CanvasView,
) -> Result<String, AssembleError> {
    let mut imports = BTreeSet::new();
    let mut bodies = Vec::with_capacity(screens.len());
    let mut placed = Vec::with_capacity(screens.len());
    for screen in screens {
        let (names, body) = split_import(&screen.source);
        imports.extend(names);
        let (component, private) = privatize(&body)?;
        bodies.push(private);
        placed.push((component, screen));
    }
    let mut out = String::new();
    if !imports.is_empty() {
        let list: Vec<&str> = imports.iter().map(String::as_str).collect();
        out.push_str(&format!(
            "import {{ {} }} from \"std-widgets.slint\";\n\n",
            list.join(", ")
        ));
    }
    for body in &bodies {
        out.push_str(body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str(&chrome(&placed, view)?);
    Ok(out)
}

/// The std-widgets import, taken off the component so the file imports each widget once.
fn split_import(source: &str) -> (BTreeSet<String>, String) {
    let mut names = BTreeSet::new();
    let mut body = String::new();
    for line in source.lines() {
        if let Some(inner) = line
            .strip_prefix("import { ")
            .and_then(|rest| rest.strip_suffix(" } from \"std-widgets.slint\";"))
        {
            for name in inner.split(", ") {
                if !name.is_empty() {
                    names.insert(name.to_owned());
                }
            }
            continue;
        }
        body.push_str(line);
        body.push('\n');
    }
    (names, body)
}

/// A file may export one component. The screens become private; the window is the export.
fn privatize(body: &str) -> Result<(String, String), AssembleError> {
    let mut component = None;
    let mut out = String::new();
    for line in body.lines() {
        if component.is_none()
            && let Some(rest) = line.strip_prefix("export component ")
        {
            let name = rest
                .split_whitespace()
                .next()
                .filter(|name| is_component_name(name))
                .ok_or(AssembleError::NoComponent)?;
            component = Some(name.to_owned());
            out.push_str("component ");
            out.push_str(rest);
            out.push('\n');
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    component
        .map(|name| (name, out))
        .ok_or(AssembleError::NoComponent)
}

fn is_component_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn number(value: f64) -> Result<String, AssembleError> {
    if !value.is_finite() {
        return Err(AssembleError::Number);
    }
    if value == 0.0 {
        return Ok("0".to_owned());
    }
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    Ok(text.to_owned())
}

fn px(value: f64) -> Result<String, AssembleError> {
    Ok(format!("{}px", number(value)?))
}

fn chrome(placed: &[(String, &CanvasScreen)], view: &CanvasView) -> Result<String, AssembleError> {
    let zoom = number(view.zoom)?;
    let pan_x = px(view.pan_x)?;
    let pan_y = px(view.pan_y)?;
    let mut extent_x: f64 = 0.0;
    let mut extent_y: f64 = 0.0;
    for (_, screen) in placed {
        extent_x = extent_x.max(screen.x + screen.width);
        extent_y = extent_y.max(screen.y + screen.height);
    }
    let selected = match view.selected_frame {
        Some(index) => index.to_string(),
        None => "-1".to_owned(),
    };
    let mut out = String::new();
    out.push_str("export component WeftStudioCanvas inherits Window {\n");
    out.push_str("    preferred-width: 960px;\n");
    out.push_str("    preferred-height: 640px;\n");
    out.push_str(&format!("    in-out property <length> pan-x: {pan_x};\n"));
    out.push_str(&format!("    in-out property <length> pan-y: {pan_y};\n"));
    out.push_str(&format!("    in-out property <float> zoom: {zoom};\n"));
    out.push_str(&format!(
        "    in-out property <int> selected-frame: {selected};\n"
    ));
    out.push_str(&format!(
        "    in-out property <string> selected-path: {};\n",
        string(&view.selected_path)
    ));
    out.push_str(&format!(
        "    in-out property <string> hover-path: {};\n",
        string(&view.hover_path)
    ));
    out.push_str("    callback perform(string, string, string);\n\n");
    out.push_str("    Flickable {\n");
    out.push_str("        x: 24px;\n");
    out.push_str("        y: 24px;\n");
    out.push_str("        width: parent.width - 24px;\n");
    out.push_str("        height: parent.height - 24px;\n");
    out.push_str("        interactive: true;\n");
    out.push_str("        mouse-drag-pan-enabled: true;\n");
    out.push_str("        content-x <=> root.pan-x;\n");
    out.push_str("        content-y <=> root.pan-y;\n");
    out.push_str(&format!(
        "        content-width: {} * root.zoom;\n",
        px(extent_x)?
    ));
    out.push_str(&format!(
        "        content-height: {} * root.zoom;\n",
        px(extent_y)?
    ));
    // The grid pans with the frames. A fixed tick count keeps the source free of a model.
    out.push_str("        for column in 24: Rectangle {\n");
    out.push_str("            x: column * 48px * root.zoom;\n");
    out.push_str("            y: 0px;\n");
    out.push_str("            width: 1px;\n");
    out.push_str(&format!(
        "            height: {} * root.zoom;\n",
        px(extent_y)?
    ));
    out.push_str("            background: #e4e4e7;\n");
    out.push_str("        }\n");
    out.push_str("        for row in 20: Rectangle {\n");
    out.push_str("            y: row * 48px * root.zoom;\n");
    out.push_str("            x: 0px;\n");
    out.push_str("            height: 1px;\n");
    out.push_str(&format!(
        "            width: {} * root.zoom;\n",
        px(extent_x)?
    ));
    out.push_str("            background: #e4e4e7;\n");
    out.push_str("        }\n");
    for (component, screen) in placed {
        out.push_str("        Rectangle {\n");
        out.push_str(&format!("            x: {} * root.zoom;\n", px(screen.x)?));
        out.push_str(&format!("            y: {} * root.zoom;\n", px(screen.y)?));
        out.push_str(&format!(
            "            width: {} * root.zoom;\n",
            px(screen.width)?
        ));
        out.push_str(&format!(
            "            height: {} * root.zoom;\n",
            px(screen.height)?
        ));
        out.push_str("            background: #ffffff;\n");
        out.push_str("            border-width: 1px;\n");
        out.push_str(&format!(
            "            border-color: root.selected-frame == {} ? #2563eb : #d4d4d8;\n",
            screen.index
        ));
        out.push_str(&format!("            {component} {{\n"));
        out.push_str("                width: 100%;\n");
        out.push_str("                height: 100%;\n");
        // `transform-scale` scales the painting where the renderer supports it. The software
        // renderer ignores it; the frame box above still grows with `zoom`.
        out.push_str("                transform-scale: root.zoom;\n");
        out.push_str(&format!(
            "                perform(action, id) => {{ root.perform({}, action, id); }}\n",
            string(&screen.screen_id)
        ));
        out.push_str("            }\n");
        out.push_str("        }\n");
    }
    out.push_str("    }\n\n");
    // Rulers stay put in the window and follow `pan-*`, so a tick lines up with the grid.
    out.push_str("    Rectangle {\n");
    out.push_str("        x: 24px;\n");
    out.push_str("        y: 0px;\n");
    out.push_str("        width: parent.width - 24px;\n");
    out.push_str("        height: 24px;\n");
    out.push_str("        background: #f4f4f5;\n");
    out.push_str("        for tick in 24: Rectangle {\n");
    out.push_str("            x: root.pan-x + tick * 48px * root.zoom;\n");
    out.push_str("            y: 16px;\n");
    out.push_str("            width: 1px;\n");
    out.push_str("            height: 8px;\n");
    out.push_str("            background: #a1a1aa;\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    Rectangle {\n");
    out.push_str("        x: 0px;\n");
    out.push_str("        y: 24px;\n");
    out.push_str("        width: 24px;\n");
    out.push_str("        height: parent.height - 24px;\n");
    out.push_str("        background: #f4f4f5;\n");
    out.push_str("        for tick in 20: Rectangle {\n");
    out.push_str("            y: root.pan-y + tick * 48px * root.zoom;\n");
    out.push_str("            x: 16px;\n");
    out.push_str("            width: 8px;\n");
    out.push_str("            height: 1px;\n");
    out.push_str("            background: #a1a1aa;\n");
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("    Rectangle {\n");
    out.push_str("        width: 24px;\n");
    out.push_str("        height: 24px;\n");
    out.push_str("        background: #f4f4f5;\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    Ok(out)
}
