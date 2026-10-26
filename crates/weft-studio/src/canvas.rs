//! The canvas of milestone 2 (T69): several open screens as frames, a pan and zoom, rulers, and a
//! selection that walks the Weft element tree. Frame positions and ruler ticks are session state. A
//! save still writes one screen's canonical markup and nothing about where the frame sat.
//!
//! Pointer coordinates are in screen space. A frame's rectangle is in canvas space:
//! `screen = canvas * zoom + pan`. Element hit-testing needs the Slint layout, which this core
//! does not have, so a point selects a frame and keyboard movement selects an element.

use std::path::{Path, PathBuf};

use weft_core::{Child, Document, Node, path_segment};

use crate::{Editor, EditorError};

/// The closest a view may zoom out, and the furthest it may zoom in.
pub const MIN_ZOOM: f64 = 0.1;
pub const MAX_ZOOM: f64 = 8.0;
const FRAME_WIDTH: f64 = 390.0;
const FRAME_HEIGHT: f64 = 800.0;
const FRAME_GAP: f64 = 80.0;
/// The smallest gap between two ruler ticks, in screen pixels.
const MIN_RULER_GAP: f64 = 48.0;
/// Past this many ticks the list is not a viewport. Returning it would not finish.
const MAX_RULER_TICKS: f64 = 1_000_000.0;

/// Where a frame sits in canvas space, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameGeom {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Which way keyboard focus moves through the element tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// The next element in document order, then the next frame.
    Next,
    /// The previous element in document order, then the previous frame.
    Previous,
    /// The first child, or no move when the element has none.
    Child,
    /// The parent element. A slot is not an element, so focus steps over it.
    Parent,
}

/// Which ruler a tick list is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// The top ruler. It reads the horizontal pan.
    Horizontal,
    /// The side ruler. It reads the vertical pan.
    Vertical,
}

/// One mark on a ruler.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    /// Position along the axis, in screen pixels, inside `[0, span]`.
    pub screen: f64,
    /// Canvas coordinate. A whole number has no decimal point.
    pub label: String,
}

struct Frame {
    editor: Editor,
    geom: FrameGeom,
}

/// One canvas. The last frame is the frontmost: a point on an overlap hits that frame.
pub struct Canvas {
    frames: Vec<Frame>,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    /// `(frame index, diagnostic path)`.
    selection: Option<(usize, String)>,
    hover: Option<(usize, String)>,
}

impl Default for Canvas {
    fn default() -> Canvas {
        Canvas::new()
    }
}

impl Canvas {
    pub fn new() -> Canvas {
        Canvas {
            frames: Vec::new(),
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            selection: None,
            hover: None,
        }
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn pan(&self) -> (f64, f64) {
        (self.pan_x, self.pan_y)
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Opens `path` as a frame to the right of the frames already open. A refused open leaves
    /// the canvas as it was.
    pub fn open_frame(&mut self, path: &Path) -> Result<usize, EditorError> {
        let editor = Editor::open(path)?;
        let x = match self
            .frames
            .iter()
            .map(|frame| frame.geom.x + frame.geom.width)
            .reduce(f64::max)
        {
            Some(edge) => edge + FRAME_GAP,
            None => 0.0,
        };
        self.frames.push(Frame {
            editor,
            geom: FrameGeom {
                x,
                y: 0.0,
                width: FRAME_WIDTH,
                height: FRAME_HEIGHT,
            },
        });
        Ok(self.frames.len() - 1)
    }

    pub fn frame(&self, index: usize) -> Option<(PathBuf, FrameGeom)> {
        self.frames
            .get(index)
            .map(|frame| (frame.editor.state().path, frame.geom))
    }

    /// Moves a frame's origin. `x` and `y` have to be finite.
    pub fn move_frame(&mut self, index: usize, x: f64, y: f64) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let Some(frame) = self.frames.get_mut(index) else {
            return false;
        };
        frame.geom.x = x;
        frame.geom.y = y;
        true
    }

    /// Shifts the view by `dx` and `dy` screen pixels.
    pub fn pan_by(&mut self, dx: f64, dy: f64) -> bool {
        if !dx.is_finite() || !dy.is_finite() {
            return false;
        }
        self.pan_x += dx;
        self.pan_y += dy;
        true
    }

    /// Multiplies the zoom by `factor`, keeping the canvas point under `(x, y)` still.
    /// `factor` has to be finite and greater than zero. The zoom stays inside
    /// [`MIN_ZOOM`] and [`MAX_ZOOM`].
    pub fn zoom_at(&mut self, factor: f64, x: f64, y: f64) -> bool {
        if !factor.is_finite() || factor <= 0.0 || !x.is_finite() || !y.is_finite() {
            return false;
        }
        let next = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let canvas_x = (x - self.pan_x) / self.zoom;
        let canvas_y = (y - self.pan_y) / self.zoom;
        self.zoom = next;
        self.pan_x = x - canvas_x * next;
        self.pan_y = y - canvas_y * next;
        true
    }

    /// Ticks for one ruler across a viewport of `span` screen pixels starting at screen 0.
    /// Horizontal uses pan_x; vertical uses pan_y. Screen position is `canvas * zoom + pan`.
    pub fn ruler(&self, axis: Axis, span: f64) -> Vec<Tick> {
        if !span.is_finite() || span <= 0.0 || !self.zoom.is_finite() || self.zoom <= 0.0 {
            return Vec::new();
        }
        let pan = match axis {
            Axis::Horizontal => self.pan_x,
            Axis::Vertical => self.pan_y,
        };
        if !pan.is_finite() {
            return Vec::new();
        }
        let step = ruler_step(self.zoom);
        let gap = step * self.zoom;
        if !gap.is_finite() || gap <= 0.0 {
            return Vec::new();
        }
        // One step of margin on each side, so a floor or ceil that lands just
        // off an in-range multiple still considers that multiple.
        let low = -pan / self.zoom;
        let high = (span - pan) / self.zoom;
        let mut index = (low / step).floor() - 1.0;
        let end = (high / step).ceil() + 1.0;
        let room = (span / gap) + 4.0;
        if !index.is_finite() || !end.is_finite() || !room.is_finite() || room > MAX_RULER_TICKS {
            return Vec::new();
        }
        let mut ticks = Vec::new();
        while index <= end {
            let canvas = index * step;
            let screen = canvas * self.zoom + pan;
            if screen >= 0.0 && screen <= span {
                ticks.push(Tick {
                    screen,
                    label: tick_label(canvas),
                });
            }
            let next = index + 1.0;
            if next <= index {
                break;
            }
            index = next;
        }
        ticks
    }

    /// The frontmost frame whose rectangle contains the screen point `(x, y)`.
    pub fn frame_at(&self, x: f64, y: f64) -> Option<usize> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let cx = (x - self.pan_x) / self.zoom;
        let cy = (y - self.pan_y) / self.zoom;
        self.frames
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, frame)| {
                let geom = frame.geom;
                (cx >= geom.x
                    && cy >= geom.y
                    && cx < geom.x + geom.width
                    && cy < geom.y + geom.height)
                    .then_some(index)
            })
    }

    /// Diagnostic paths of one frame's elements, in document order.
    pub fn elements(&self, index: usize) -> Option<Vec<String>> {
        self.frames
            .get(index)
            .map(|frame| element_order(frame.editor.document()))
    }

    pub fn selection(&self) -> Option<(usize, &str)> {
        self.selection
            .as_ref()
            .map(|(index, path)| (*index, path.as_str()))
    }

    pub fn hover(&self) -> Option<(usize, &str)> {
        self.hover
            .as_ref()
            .map(|(index, path)| (*index, path.as_str()))
    }

    /// Selects the element at `path` in `index`. An unknown frame or path does nothing.
    pub fn select(&mut self, index: usize, path: &str) -> bool {
        if self
            .elements(index)
            .is_some_and(|paths| paths.iter().any(|candidate| candidate == path))
        {
            self.selection = Some((index, path.to_owned()));
            true
        } else {
            false
        }
    }

    /// Hovers the element at `path`, or clears the hover when `path` is `None`.
    pub fn hover_at(&mut self, index: usize, path: Option<&str>) -> bool {
        match path {
            None => {
                self.hover = None;
                true
            }
            Some(path)
                if self
                    .elements(index)
                    .is_some_and(|paths| paths.iter().any(|candidate| candidate == path)) =>
            {
                self.hover = Some((index, path.to_owned()));
                true
            }
            Some(_) => false,
        }
    }

    /// Moves the selection. With nothing selected, `Next` starts at the first element and
    /// `Previous` at the last. At either end the selection stays and this returns false.
    pub fn focus(&mut self, move_: Focus) -> bool {
        let paths = self.element_index();
        if paths.is_empty() {
            return false;
        }
        let current = self.selection.as_ref().and_then(|(index, path)| {
            paths
                .iter()
                .position(|(i, candidate)| i == index && candidate == path)
        });
        let next = match move_ {
            Focus::Next => match current {
                Some(index) if index + 1 < paths.len() => Some(index + 1),
                Some(_) => None,
                None => Some(0),
            },
            Focus::Previous => match current {
                Some(0) => None,
                Some(index) => Some(index - 1),
                None => Some(paths.len() - 1),
            },
            Focus::Child => {
                let Some((index, path)) = self.selection.clone() else {
                    return false;
                };
                return self.select_child(index, &path);
            }
            Focus::Parent => {
                let Some((index, path)) = self.selection.clone() else {
                    return false;
                };
                let Some(parent) = parent_path(&path) else {
                    return false;
                };
                return self.select(index, &parent);
            }
        };
        let Some(index) = next else {
            return false;
        };
        self.selection = Some(paths[index].clone());
        true
    }

    fn element_index(&self) -> Vec<(usize, String)> {
        self.frames
            .iter()
            .enumerate()
            .flat_map(|(index, frame)| {
                element_order(frame.editor.document())
                    .into_iter()
                    .map(move |path| (index, path))
            })
            .collect()
    }

    fn select_child(&mut self, index: usize, path: &str) -> bool {
        let Some(paths) = self.elements(index) else {
            return false;
        };
        let Some(child) = paths
            .into_iter()
            .find(|candidate| parent_path(candidate).as_deref() == Some(path))
        else {
            return false;
        };
        self.selection = Some((index, child));
        true
    }
}

fn element_order(document: &Document) -> Vec<String> {
    let root = &document.root;
    let mut out = Vec::new();
    walk(
        root,
        &format!("/{}", path_segment(&root.kind, root.id.as_deref(), None)),
        &mut out,
    );
    out
}

fn walk(node: &Node, path: &str, out: &mut Vec<String>) {
    out.push(path.to_owned());
    for (index, child) in node.children.iter().enumerate() {
        if let Child::Node(inner) = child {
            let child_path = format!(
                "{path}/{}",
                path_segment(&inner.kind, inner.id.as_deref(), Some(index))
            );
            walk(inner, &child_path, out);
        }
    }
    for (name, list) in &node.slots {
        let slot = format!("{path}/slot[{name}]");
        for (index, child) in list.iter().enumerate() {
            if let Child::Node(inner) = child {
                let child_path = format!(
                    "{slot}/{}",
                    path_segment(&inner.kind, inner.id.as_deref(), Some(index))
                );
                walk(inner, &child_path, out);
            }
        }
    }
}

/// The parent element path. A `slot[name]` segment is a list, not an element, so it is skipped.
fn parent_path(path: &str) -> Option<String> {
    let (parent, _) = path.rsplit_once('/')?;
    if parent.is_empty() {
        return None;
    }
    if parent
        .rsplit('/')
        .next()
        .is_some_and(|segment| segment.starts_with("slot["))
    {
        let (above, _) = parent.rsplit_once('/')?;
        if above.is_empty() {
            return None;
        }
        return Some(above.to_owned());
    }
    Some(parent.to_owned())
}

/// Smallest step of the form 1, 2 or 5 times a power of ten with `step * zoom >= 48`.
fn ruler_step(zoom: f64) -> f64 {
    let needed = MIN_RULER_GAP / zoom;
    let mut exponent = if needed.is_finite() && needed > 0.0 {
        let decade = needed.log10().floor();
        if decade.is_finite() {
            // One decade below the estimate, so a `log10` that rounds up still
            // walks the candidate that actually fits.
            (decade.clamp(-300.0, 300.0) as i32).saturating_sub(1)
        } else {
            0
        }
    } else {
        0
    };
    for _ in 0..16 {
        let magnitude = 10_f64.powi(exponent);
        if !magnitude.is_finite() {
            break;
        }
        for factor in [1.0, 2.0, 5.0] {
            let step = factor * magnitude;
            if step.is_finite() && step * zoom >= MIN_RULER_GAP {
                return step;
            }
        }
        exponent = exponent.saturating_add(1);
    }
    needed
}

/// A whole canvas coordinate has no decimal point. Anything else uses ordinary display.
fn tick_label(value: f64) -> String {
    let nearest = value.round();
    if (value - nearest).abs() > 1e-6 || !nearest.is_finite() || nearest.abs() >= 1e15 {
        return value.to_string();
    }
    let digits = format!("{:.0}", nearest.abs());
    if nearest.is_sign_negative() && nearest != 0.0 {
        format!("-{digits}")
    } else {
        digits
    }
}
