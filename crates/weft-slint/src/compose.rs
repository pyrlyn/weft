//! One Slint file for a host window that places several embedded screens (T69 milestone 2).
//! The components come from [`crate::generate_embedded`]; this module only assembles them.

use std::collections::BTreeSet;

use weft_core::js_number;

use crate::EmbeddedComponent;

/// One embedded screen in the host window. Lengths are logical pixels.
#[derive(Debug, Clone, Copy)]
pub struct CanvasFrame<'a> {
    pub component: &'a EmbeddedComponent,
    pub x: f64,
    pub y: f64,
    /// The frame's width at zoom 1. The component's preferred height gives the height.
    pub width: f64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ComposeError {
    #[error("two frames are named {0}")]
    Duplicate(String),
    #[error("a frame position or width is not a finite length")]
    Length,
}

/// A `.slint` file whose only export is `Canvas`, a `Window` holding each frame.
/// `pan-x`, `pan-y` and `zoom` move and scale the frames without regenerating the file.
/// A left-pointer drag on the window background, which sits behind the frames, changes
/// `pan-x` and `pan-y`. A `MenuBar` is not here: a screen with a menu never becomes an
/// [`EmbeddedComponent`].
pub fn compose(frames: &[CanvasFrame<'_>]) -> Result<String, ComposeError> {
    let mut names = BTreeSet::new();
    let mut imports = BTreeSet::new();
    for frame in frames {
        if !frame.x.is_finite()
            || !frame.y.is_finite()
            || !(frame.width.is_finite() && frame.width > 0.0)
        {
            return Err(ComposeError::Length);
        }
        if !names.insert(frame.component.name.as_str()) {
            return Err(ComposeError::Duplicate(frame.component.name.clone()));
        }
        imports.extend(frame.component.imports.iter().copied());
    }

    let mut out = String::new();
    if !imports.is_empty() {
        let list: Vec<&str> = imports.into_iter().collect();
        out.push_str(&format!(
            "import {{ {} }} from \"std-widgets.slint\";\n\n",
            list.join(", ")
        ));
    }
    for frame in frames {
        out.push_str(frame.component.source.trim_end());
        out.push_str("\n\n");
    }
    out.push_str(
        "\
export component Canvas inherits Window {
    in-out property <length> pan-x: 0px;
    in-out property <length> pan-y: 0px;
    in-out property <float> zoom: 1;
    property <length> pan-origin-x;
    property <length> pan-origin-y;

    // Behind the frames, so a control that takes the pointer keeps it.
    touch := TouchArea {
        width: 100%;
        height: 100%;
        pointer-event(event) => {
            if (event.kind == PointerEventKind.down && event.button == PointerEventButton.left) {
                root.pan-origin-x = root.pan-x;
                root.pan-origin-y = root.pan-y;
            }
        }
        moved => {
            if (self.pressed) {
                root.pan-x = root.pan-origin-x + self.mouse-x - self.pressed-x;
                root.pan-y = root.pan-origin-y + self.mouse-y - self.pressed-y;
            }
        }
    }
",
    );
    for (index, frame) in frames.iter().enumerate() {
        let x = js_number(frame.x);
        let y = js_number(frame.y);
        let width = js_number(frame.width);
        out.push_str(&format!(
            "    frame-{index} := {} {{\n        x: {x}px + root.pan-x;\n        y: {y}px + root.pan-y;\n        width: {width}px * root.zoom;\n    }}\n",
            frame.component.name
        ));
    }
    out.push_str("}\n");
    Ok(out)
}
