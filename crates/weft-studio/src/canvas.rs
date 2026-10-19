//! Several `.weft` screens as frames on one canvas (T69 milestone 2, the document core).
//! The shells still show one screen through `Editor`; this type is what a later shell call will
//! draw. Pan and zoom live here and in the generated window. Selection and hover name elements by
//! the same paths the diagnostics use (SPEC §6.1), in document order, which is the keyboard order.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use weft_core::{Child, Document, Node, path_segment};
use weft_slint::{AssembleError, CanvasScreen, CanvasView, assemble_canvas, type_name};

use crate::{Editor, EditorError, RenderInput};

/// A frame's size, the one T69.1 measured open-to-first-frame at. The gap is one grid step.
pub const FRAME_WIDTH: f64 = 480.0;
pub const FRAME_HEIGHT: f64 = 800.0;
pub const FRAME_GAP: f64 = 48.0;

const ZOOM_MIN: f64 = 0.25;
const ZOOM_MAX: f64 = 4.0;

#[derive(Debug, thiserror::Error)]
pub enum CanvasError {
    #[error(transparent)]
    Editor(#[from] EditorError),
    #[error("a canvas needs at least one .weft file")]
    NoScreens,
    #[error("no .weft file in {}", .0.display())]
    Empty(PathBuf),
    #[error("no frame {0}")]
    Frame(usize),
    #[error("no element {path} on frame {frame}")]
    Element { frame: usize, path: String },
    #[error("no screen on the canvas can be drawn: {0}")]
    Draw(String),
    #[error(transparent)]
    Assemble(#[from] AssembleError),
}

/// Which way keyboard navigation moves the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Next,
    Previous,
    /// The parent element. The screen stays selected.
    Parent,
    NextFrame,
    PreviousFrame,
}

/// An element on a frame, named by its diagnostic path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ElementRef {
    pub frame: usize,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct FrameState {
    pub path: PathBuf,
    pub title: String,
    pub screen: Option<String>,
    pub x: f64,
    pub y: f64,
    /// False when the screen cannot sit in the canvas window (a menu, or it does not validate).
    pub embedded: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CanvasState {
    pub frames: Vec<FrameState>,
    /// The `Flickable` content offset, in logical pixels. Zero is the origin.
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
    pub selection: Option<ElementRef>,
    pub hover: Option<ElementRef>,
}

struct Frame {
    editor: Editor,
    /// A single token, so the source comment's `name=` stays one word.
    stem: String,
    x: f64,
    y: f64,
}

pub struct Canvas {
    frames: Vec<Frame>,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    selection: Option<ElementRef>,
    hover: Option<ElementRef>,
}

impl Canvas {
    /// One frame per path, in the order given. A syntax error refuses the whole open, as `Editor`
    /// does for one file.
    pub fn open_files<P: AsRef<Path>>(paths: &[P]) -> Result<Canvas, CanvasError> {
        if paths.is_empty() {
            return Err(CanvasError::NoScreens);
        }
        let mut frames = Vec::with_capacity(paths.len());
        for (index, path) in paths.iter().enumerate() {
            let path = path.as_ref();
            frames.push(Frame {
                editor: Editor::open(path)?,
                stem: stem_of(path),
                x: index as f64 * (FRAME_WIDTH + FRAME_GAP),
                y: 0.0,
            });
        }
        Ok(Canvas::new(frames))
    }

    /// Every `.weft` file directly in `dir`, in file-name order. Subdirectories stay out.
    pub fn open_dir(dir: &Path) -> Result<Canvas, CanvasError> {
        let entries =
            std::fs::read_dir(dir).map_err(|error| EditorError::Read(dir.to_owned(), error))?;
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| EditorError::Read(dir.to_owned(), error))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "weft") && path.is_file() {
                paths.push(path);
            }
        }
        paths.sort();
        if paths.is_empty() {
            return Err(CanvasError::Empty(dir.to_owned()));
        }
        Canvas::open_files(&paths)
    }

    fn new(frames: Vec<Frame>) -> Canvas {
        Canvas {
            frames,
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            selection: None,
            hover: None,
        }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Sets the content offset. A non-finite value leaves that axis as it was.
    pub fn set_pan(&mut self, x: f64, y: f64) {
        if x.is_finite() {
            self.pan_x = x;
        }
        if y.is_finite() {
            self.pan_y = y;
        }
    }

    /// Clamps to 1/4 through 4. A non-finite value leaves the zoom as it was.
    pub fn set_zoom(&mut self, zoom: f64) {
        if zoom.is_finite() {
            self.zoom = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        }
    }

    pub fn paths(&self, frame: usize) -> Result<Vec<String>, CanvasError> {
        let frame = self.frames.get(frame).ok_or(CanvasError::Frame(frame))?;
        Ok(element_order(frame.editor.document()))
    }

    pub fn select(&mut self, frame: usize, path: &str) -> Result<(), CanvasError> {
        self.require(frame, path)?;
        self.selection = Some(ElementRef {
            frame,
            path: path.to_owned(),
        });
        Ok(())
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn set_hover(&mut self, frame: usize, path: &str) -> Result<(), CanvasError> {
        self.require(frame, path)?;
        self.hover = Some(ElementRef {
            frame,
            path: path.to_owned(),
        });
        Ok(())
    }

    pub fn clear_hover(&mut self) {
        self.hover = None;
    }

    /// Moves the selection. With nothing selected, `Next` and `NextFrame` start at the first
    /// element and the other directions stay put.
    pub fn move_selection(&mut self, direction: Direction) -> Option<&str> {
        let paths: Vec<Vec<String>> = self
            .frames
            .iter()
            .map(|frame| element_order(frame.editor.document()))
            .collect();
        let next = match direction {
            Direction::Next => step(&paths, self.selection.as_ref(), 1),
            Direction::Previous => step(&paths, self.selection.as_ref(), -1),
            Direction::NextFrame => step_frame(&paths, self.selection.as_ref(), 1),
            Direction::PreviousFrame => step_frame(&paths, self.selection.as_ref(), -1),
            Direction::Parent => parent_of(&paths, self.selection.as_ref()),
        };
        if let Some(found) = next {
            self.selection = Some(found);
        }
        self.selection.as_ref().map(|element| element.path.as_str())
    }

    pub fn state(&self) -> CanvasState {
        let placed = self.place();
        CanvasState {
            frames: self
                .frames
                .iter()
                .zip(&placed)
                .map(|(frame, place)| {
                    let state = frame.editor.state();
                    let error = place.as_ref().err().cloned();
                    FrameState {
                        path: state.path,
                        title: state.title,
                        screen: state.screen,
                        x: frame.x,
                        y: frame.y,
                        embedded: error.is_none(),
                        error,
                    }
                })
                .collect(),
            pan_x: self.pan_x,
            pan_y: self.pan_y,
            zoom: self.zoom,
            selection: self.selection.clone(),
            hover: self.hover.clone(),
        }
    }

    /// One Slint window of every screen that can be embedded. A menu, or a screen that does not
    /// validate, stays in [`Self::state`] with `embedded: false` and is left out of the source.
    /// Sample data stays on `Editor::render_input`: the canvas shows the screens without it.
    pub fn render_input(&self) -> Result<RenderInput, CanvasError> {
        let placed = self.place();
        let mut screens = Vec::new();
        let mut first_error = None;
        for (index, (frame, place)) in self.frames.iter().zip(&placed).enumerate() {
            match place {
                Ok(source) => screens.push(CanvasScreen {
                    source: source.clone(),
                    screen_id: frame.editor.state().screen.unwrap_or_default(),
                    index,
                    x: frame.x,
                    y: frame.y,
                    width: FRAME_WIDTH,
                    height: FRAME_HEIGHT,
                }),
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error.clone());
                    }
                }
            }
        }
        if screens.is_empty() {
            return Err(CanvasError::Draw(
                first_error.unwrap_or_else(|| "nothing to draw".to_owned()),
            ));
        }
        let source = assemble_canvas(
            &screens,
            &CanvasView {
                pan_x: self.pan_x,
                pan_y: self.pan_y,
                zoom: self.zoom,
                selected_frame: self.selection.as_ref().map(|element| element.frame),
                selected_path: self
                    .selection
                    .as_ref()
                    .map(|element| element.path.clone())
                    .unwrap_or_default(),
                hover_path: self
                    .hover
                    .as_ref()
                    .map(|element| element.path.clone())
                    .unwrap_or_default(),
            },
        )?;
        Ok(RenderInput { source, data: None })
    }

    /// Saves one frame. `None` writes its current path.
    pub fn save(&mut self, frame: usize, path: Option<&Path>) -> Result<(), CanvasError> {
        let frame = self
            .frames
            .get_mut(frame)
            .ok_or(CanvasError::Frame(frame))?;
        frame.editor.save(path)?;
        Ok(())
    }

    fn require(&self, frame: usize, path: &str) -> Result<(), CanvasError> {
        let paths = self.paths(frame)?;
        if paths.iter().any(|candidate| candidate == path) {
            Ok(())
        } else {
            Err(CanvasError::Element {
                frame,
                path: path.to_owned(),
            })
        }
    }

    /// The embedded source of each frame, or why that frame was left out. Stems that would name
    /// one Slint component (`foo-bar` and `foo_bar`) take `frame-<index>` instead.
    fn place(&self) -> Vec<Result<String, String>> {
        let mut used = HashSet::new();
        let mut stems = Vec::with_capacity(self.frames.len());
        for (index, frame) in self.frames.iter().enumerate() {
            let mut stem = frame.stem.clone();
            let mut extra = 2u32;
            while !used.insert(format!("{}Screen", type_name(&stem))) {
                stem = format!("frame-{index}-{extra}");
                extra += 1;
            }
            stems.push(stem);
        }
        self.frames
            .iter()
            .zip(stems)
            .map(|(frame, stem)| {
                frame
                    .editor
                    .embedded_source(Some(&stem))
                    .map_err(|error| error.to_string())
            })
            .collect()
    }
}

fn stem_of(path: &Path) -> String {
    let raw = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut stem = String::new();
    for c in raw.chars() {
        if c.is_whitespace() {
            stem.push('-');
        } else {
            stem.push(c);
        }
    }
    if stem.is_empty() {
        stem.push_str("screen");
    }
    stem
}

/// Elements in document order: a node, then its children, then its slots. The paths are the
/// diagnostic paths, so a selection is a patch target.
fn element_order(document: &Document) -> Vec<String> {
    let mut paths = Vec::new();
    let root = &document.root;
    let path = format!("/{}", path_segment(&root.kind, root.id.as_deref(), None));
    paths.push(path.clone());
    walk(root, &path, &mut paths);
    paths
}

fn walk(node: &Node, path: &str, paths: &mut Vec<String>) {
    walk_list(&node.children, path, paths);
    for (name, list) in &node.slots {
        walk_list(list, &format!("{path}/slot[{name}]"), paths);
    }
}

fn walk_list(list: &[Child], path: &str, paths: &mut Vec<String>) {
    for (index, child) in list.iter().enumerate() {
        let Some(node) = child.as_node() else {
            continue;
        };
        let here = format!(
            "{path}/{}",
            path_segment(&node.kind, node.id.as_deref(), Some(index))
        );
        paths.push(here.clone());
        walk(node, &here, paths);
    }
}

fn step(paths: &[Vec<String>], current: Option<&ElementRef>, delta: isize) -> Option<ElementRef> {
    let flat = flatten(paths);
    if flat.is_empty() {
        return None;
    }
    let index = current
        .and_then(|element| {
            flat.iter()
                .position(|(frame, path)| *frame == element.frame && *path == &element.path)
        })
        .map(|found| found as isize);
    let index = match (index, delta) {
        (None, 1) => 0,
        (None, _) => return None,
        (Some(found), _) => found + delta,
    };
    let len = flat.len() as isize;
    let index = index.rem_euclid(len) as usize;
    let (frame, path) = &flat[index];
    Some(ElementRef {
        frame: *frame,
        path: (*path).clone(),
    })
}

fn step_frame(
    paths: &[Vec<String>],
    current: Option<&ElementRef>,
    delta: isize,
) -> Option<ElementRef> {
    if paths.is_empty() {
        return None;
    }
    let start = current.map(|element| element.frame as isize);
    let index = match (start, delta) {
        (None, 1) => 0,
        (None, _) => return None,
        (Some(found), _) => found + delta,
    };
    let len = paths.len() as isize;
    let index = index.rem_euclid(len) as usize;
    let path = paths.get(index)?.first()?.clone();
    Some(ElementRef { frame: index, path })
}

fn parent_of(paths: &[Vec<String>], current: Option<&ElementRef>) -> Option<ElementRef> {
    let element = current?;
    let list = paths.get(element.frame)?;
    let parent = list
        .iter()
        .filter(|path| element.path.starts_with(&format!("{path}/")))
        .max_by_key(|path| path.len())?
        .clone();
    Some(ElementRef {
        frame: element.frame,
        path: parent,
    })
}

fn flatten(paths: &[Vec<String>]) -> Vec<(usize, &String)> {
    paths
        .iter()
        .enumerate()
        .flat_map(|(frame, list)| list.iter().map(move |path| (frame, path)))
        .collect()
}
