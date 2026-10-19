//! Slint for Weft (SPEC §9, "To Slint" and "From Slint"): `generate` prints one `.slint`
//! component per screen for Slint 1.x and its `std-widgets.slint`, and `import_slint` reads such a
//! file back into the Weft document it came from. `generate_embedded` prints the same screen as a
//! component that does not inherit `Window`, so a host can place several screens in one surface.
//! `read_slint` (feature `import`) reads a file `import_slint` rejects, from the Slint syntax tree.
//! `runtime` (feature `runtime`) compiles the generated source with the Slint interpreter and fills
//! it with sample data. The mapping is documented in this crate's README.

mod compose;
mod data;
mod generate;
mod import;
mod names;
#[cfg(feature = "import")]
mod read;
#[cfg(feature = "import")]
mod read_expr;
#[cfg(feature = "runtime")]
pub mod runtime;

pub use compose::{CanvasFrame, ComposeError, compose};
pub use generate::{
    EmbeddedComponent, GenerateError, GenerateOptions, generate, generate_embedded,
};
pub use import::{ImportError, ImportOptions, MAX_SOURCE_LENGTH, import_slint};
#[cfg(feature = "import")]
pub use read::read_slint;
#[cfg(feature = "import")]
pub use weft_import::{ImportResult, Loss, LossKind};

/// Something a valid document says that the generator cannot express in Slint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsupported {
    /// The element path (SPEC §6.1) or the binding path the problem is about.
    pub path: String,
    pub message: String,
}

impl Unsupported {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Unsupported {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}
