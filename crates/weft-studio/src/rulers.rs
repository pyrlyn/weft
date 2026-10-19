//! Rulers for the canvas (T69 milestone 2). Tick positions are session state: a save still
//! writes one screen's canonical markup and nothing about the view.
//!
//! Pointer coordinates are in screen space. A design coordinate is in canvas space, and the two
//! meet by `screen = canvas * zoom + pan`. The horizontal ruler uses `pan_x` and the viewport
//! width; the vertical ruler uses `pan_y` and the viewport height. Drawing the ticks is the
//! Slint surface, which this crate does not link.

/// The closest two major ticks may sit, in screen pixels. The step is the next 1-2-5 size that
/// keeps at least this gap.
const MIN_MAJOR_PX: f64 = 48.0;
/// Minor ticks are omitted when they would sit closer than this.
const MIN_MINOR_PX: f64 = 8.0;
/// A viewport this dense is drawn with a coarser step instead of a longer list.
const MAX_TICKS: usize = 4096;
/// `i64` converted to `f64` is exact inside this magnitude.
const EXACT_INDEX: f64 = (1_i64 << 53) as f64;

/// A major tick is labelled. A minor tick is the subdivision between two majors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickRank {
    Major,
    Minor,
}

/// One mark on a ruler.
#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    /// Distance from the left or the top of the viewport, in screen pixels.
    pub screen: f64,
    /// The design coordinate the mark names.
    pub canvas: f64,
    pub rank: TickRank,
    /// Set on a major tick.
    pub label: Option<String>,
}

/// The ticks that fill one viewport edge.
#[derive(Clone, Debug, PartialEq)]
pub struct Ruler {
    /// Canvas units between major ticks.
    pub step: f64,
    pub ticks: Vec<Tick>,
}

/// Both edges of one viewport. They share a zoom and a step.
#[derive(Clone, Debug, PartialEq)]
pub struct Rulers {
    pub horizontal: Ruler,
    pub vertical: Ruler,
}

/// Where a frame covers a ruler, clipped to the viewport. `start` and `end` are screen pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RulerError {
    #[error("the pan, zoom, length or coordinate is not finite")]
    NotFinite,
    #[error("the zoom is not positive")]
    Zoom,
    #[error("the length is negative")]
    Length,
    #[error("the extent is negative")]
    Extent,
    #[error("the ruler would need more than {MAX_TICKS} ticks")]
    TooMany,
}

/// `screen = canvas * zoom + pan`. `None` when an input is not finite or the zoom is not positive.
#[must_use]
pub fn screen_at(canvas: f64, pan: f64, zoom: f64) -> Option<f64> {
    if !canvas.is_finite() || !mapped(pan, zoom) {
        return None;
    }
    let screen = canvas.mul_add(zoom, pan);
    screen.is_finite().then_some(screen)
}

/// The inverse of [`screen_at`].
#[must_use]
pub fn canvas_at(screen: f64, pan: f64, zoom: f64) -> Option<f64> {
    if !screen.is_finite() || !mapped(pan, zoom) {
        return None;
    }
    let canvas = (screen - pan) / zoom;
    canvas.is_finite().then_some(canvas)
}

/// The canvas coordinate under a screen position, written the way the major ticks are labelled,
/// with one extra digit so a point between ticks still shows.
pub fn readout(screen: f64, pan: f64, zoom: f64) -> Result<String, RulerError> {
    view(pan, zoom, 0.0)?;
    let canvas = canvas_at(screen, pan, zoom).ok_or(RulerError::NotFinite)?;
    let (man, exp) = major_step(zoom);
    let places = readout_places(man, exp, zoom);
    let scale = 10_f64.powi(places);
    let scaled = (canvas * scale).round();
    if !scaled.is_finite() || scaled.abs() >= EXACT_INDEX {
        return Err(RulerError::NotFinite);
    }
    Ok(label(scaled as i64, 1, -places))
}

/// Major and minor ticks along one edge. `length` is the viewport width or height.
pub fn ruler(pan: f64, zoom: f64, length: f64) -> Result<Ruler, RulerError> {
    view(pan, zoom, length)?;
    let (mut man, mut exp) = major_step(zoom);
    for _ in 0..40 {
        if let Some(ticks) = collect(pan, zoom, length, man, exp)? {
            return Ok(Ruler {
                step: (f64::from(man)) * 10_f64.powi(exp),
                ticks,
            });
        }
        (man, exp) = coarsen(man, exp);
    }
    Err(RulerError::TooMany)
}

/// The horizontal ruler over `width` and the vertical ruler over `height`.
pub fn rulers(
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    width: f64,
    height: f64,
) -> Result<Rulers, RulerError> {
    Ok(Rulers {
        horizontal: ruler(pan_x, zoom, width)?,
        vertical: ruler(pan_y, zoom, height)?,
    })
}

/// The screen interval a frame covers on one ruler, clipped to the viewport. `None` when the
/// frame misses the edge. A zero extent is a point, and it is kept when it lies on the edge.
pub fn span(
    origin: f64,
    extent: f64,
    pan: f64,
    zoom: f64,
    length: f64,
) -> Result<Option<Span>, RulerError> {
    view(pan, zoom, length)?;
    if !origin.is_finite() || !extent.is_finite() {
        return Err(RulerError::NotFinite);
    }
    if extent < 0.0 {
        return Err(RulerError::Extent);
    }
    let start = origin.mul_add(zoom, pan);
    let end = (origin + extent).mul_add(zoom, pan);
    if !start.is_finite() || !end.is_finite() {
        return Err(RulerError::NotFinite);
    }
    let (lo, hi) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    if hi < 0.0 || lo > length {
        return Ok(None);
    }
    Ok(Some(Span {
        start: lo.max(0.0),
        end: hi.min(length),
    }))
}

fn mapped(pan: f64, zoom: f64) -> bool {
    pan.is_finite() && zoom.is_finite() && zoom > 0.0
}

fn view(pan: f64, zoom: f64, length: f64) -> Result<(), RulerError> {
    if !pan.is_finite() || !zoom.is_finite() || !length.is_finite() {
        return Err(RulerError::NotFinite);
    }
    if zoom <= 0.0 {
        return Err(RulerError::Zoom);
    }
    if length < 0.0 {
        return Err(RulerError::Length);
    }
    Ok(())
}

/// `(mantissa, exponent)` with mantissa in {1, 2, 5} and value `mantissa * 10^exponent`.
fn major_step(zoom: f64) -> (u32, i32) {
    let raw = MIN_MAJOR_PX / zoom;
    if !raw.is_finite() || raw <= 0.0 {
        return (1, 30);
    }
    let exp_f = raw.log10().floor();
    if !exp_f.is_finite() || exp_f >= 30.0 {
        return (1, 30);
    }
    if exp_f <= -12.0 {
        return (1, -12);
    }
    let exp = exp_f as i32;
    let mantissa_f = raw / 10_f64.powi(exp);
    if mantissa_f <= 1.0 {
        (1, exp)
    } else if mantissa_f <= 2.0 {
        (2, exp)
    } else if mantissa_f <= 5.0 {
        (5, exp)
    } else {
        (1, exp + 1)
    }
}

fn minor_spec(mantissa: u32, exp: i32) -> (u32, i32, u32) {
    match mantissa {
        1 => (2, exp - 1, 5),
        2 => (5, exp - 1, 4),
        _ => (1, exp, 5),
    }
}

fn coarsen(mantissa: u32, exp: i32) -> (u32, i32) {
    match mantissa {
        1 => (2, exp),
        2 => (5, exp),
        _ => (1, exp.saturating_add(1)),
    }
}

fn readout_places(mantissa: u32, exp: i32, zoom: f64) -> i32 {
    let (mman, mexp, _) = minor_spec(mantissa, exp);
    let gap = f64::from(mman) * 10_f64.powi(mexp) * zoom;
    let base = if gap >= MIN_MINOR_PX { mexp } else { exp };
    let decimals = if base < 0 { -base } else { 0 };
    decimals.saturating_add(1).clamp(1, 6)
}

/// `None` means the step is still too fine and the caller should coarsen it.
fn collect(
    pan: f64,
    zoom: f64,
    length: f64,
    man: u32,
    exp: i32,
) -> Result<Option<Vec<Tick>>, RulerError> {
    let (mman, mexp, every) = minor_spec(man, exp);
    let gap = f64::from(mman) * 10_f64.powi(mexp) * zoom;
    if gap >= MIN_MINOR_PX
        && let Some(ticks) = build(pan, zoom, length, mman, mexp, every)?
    {
        return Ok(Some(ticks));
    }
    build(pan, zoom, length, man, exp, 1)
}

fn build(
    pan: f64,
    zoom: f64,
    length: f64,
    man: u32,
    exp: i32,
    every: u32,
) -> Result<Option<Vec<Tick>>, RulerError> {
    let quantum = f64::from(man) * 10_f64.powi(exp);
    if quantum <= 0.0 || !quantum.is_finite() {
        return Err(RulerError::NotFinite);
    }
    let first = ((-pan / zoom) / quantum).ceil();
    let last = ((length - pan) / zoom / quantum).floor();
    if !first.is_finite()
        || !last.is_finite()
        || first.abs() >= EXACT_INDEX
        || last.abs() >= EXACT_INDEX
    {
        return Err(RulerError::NotFinite);
    }
    if last < first {
        return Ok(Some(Vec::new()));
    }
    if last - first + 1.0 > MAX_TICKS as f64 {
        return Ok(None);
    }
    let every_i = i64::from(every);
    let mut ticks = Vec::new();
    let mut index = first as i64;
    let last_i = last as i64;
    while index <= last_i {
        // `index` is inside 2^53, so the conversion is exact.
        let canvas = index as f64 * quantum;
        let screen = canvas.mul_add(zoom, pan);
        if (-1e-6..=length + 1e-6).contains(&screen) {
            let major = index.rem_euclid(every_i) == 0;
            ticks.push(Tick {
                screen,
                canvas,
                rank: if major {
                    TickRank::Major
                } else {
                    TickRank::Minor
                },
                label: major.then(|| label(index, man, exp)),
            });
        }
        index += 1;
    }
    Ok(Some(ticks))
}

fn label(index: i64, mantissa: u32, exp: i32) -> String {
    let significand = i128::from(index) * i128::from(mantissa);
    if significand == 0 {
        return "0".to_string();
    }
    if !(-12..=24).contains(&exp) {
        return format!("{significand}e{exp}");
    }
    let negative = significand < 0;
    let digits = significand.unsigned_abs().to_string();
    let mut body = if exp >= 0 {
        let mut out = digits;
        out.extend(std::iter::repeat_n('0', exp as usize));
        out
    } else {
        let places = (-exp) as usize;
        if digits.len() > places {
            let split = digits.len() - places;
            let mut out = digits;
            out.insert(split, '.');
            out
        } else {
            let mut out = String::from("0.");
            out.extend(std::iter::repeat_n('0', places - digits.len()));
            out.push_str(&digits);
            out
        }
    };
    if let Some(dot) = body.find('.') {
        let trimmed = body[dot + 1..].trim_end_matches('0').len();
        body.truncate(dot + 1 + trimmed);
        if body.ends_with('.') {
            body.pop();
        }
    }
    if negative {
        body.insert(0, '-');
    }
    body
}
