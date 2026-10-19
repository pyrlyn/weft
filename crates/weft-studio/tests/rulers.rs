//! Tick positions for one viewport edge. No Slint: the marks are numbers a surface can draw.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use weft_studio::{RulerError, TickRank, canvas_at, readout, ruler, rulers, screen_at, span};

fn near(left: f64, right: f64) {
    assert!((left - right).abs() < 1e-9, "{left} != {right}");
}

fn labels(rank_major_only: bool, ticks: &[weft_studio::Tick]) -> Vec<&str> {
    ticks
        .iter()
        .filter(|tick| !rank_major_only || tick.rank == TickRank::Major)
        .filter_map(|tick| tick.label.as_deref())
        .collect()
}

#[test]
fn majors_land_on_a_one_two_five_step_and_minors_fill_the_gap() {
    let ruler = ruler(0.0, 1.0, 200.0).unwrap();
    near(ruler.step, 50.0);
    assert_eq!(labels(true, &ruler.ticks), ["0", "50", "100", "150", "200"]);
    let minor = ruler
        .ticks
        .iter()
        .find(|tick| tick.rank == TickRank::Minor)
        .unwrap();
    near(minor.canvas, 10.0);
    near(minor.screen, 10.0);
    assert!(minor.label.is_none());
    for pair in ruler.ticks.windows(2) {
        assert!(pair[1].screen - pair[0].screen >= 8.0 - 1e-9);
        near(pair[0].screen, pair[0].canvas * 1.0 + 0.0);
    }
}

#[test]
fn a_pan_shifts_the_ticks_and_a_zoom_scales_them() {
    let shifted = ruler(25.0, 1.0, 100.0).unwrap();
    let zero = shifted
        .ticks
        .iter()
        .find(|tick| tick.label.as_deref() == Some("0"))
        .unwrap();
    near(zero.screen, 25.0);
    let below = shifted
        .ticks
        .iter()
        .find(|tick| (tick.canvas + 20.0).abs() < 1e-9)
        .unwrap();
    assert_eq!(below.rank, TickRank::Minor);
    assert!(below.label.is_none());
    near(below.screen, 5.0);
    assert!(
        shifted
            .ticks
            .iter()
            .all(|tick| tick.rank == TickRank::Minor || tick.canvas >= 0.0)
    );
    let at_fifty = shifted
        .ticks
        .iter()
        .find(|tick| tick.label.as_deref() == Some("50"))
        .unwrap();
    near(at_fifty.screen, 75.0);

    let zoomed = ruler(0.0, 2.0, 100.0).unwrap();
    near(zoomed.step, 50.0);
    let fifty = zoomed
        .ticks
        .iter()
        .find(|tick| tick.label.as_deref() == Some("50"))
        .unwrap();
    near(fifty.screen, 100.0);

    let close = ruler(0.0, 8.0, 80.0).unwrap();
    near(close.step, 10.0);
    assert_eq!(labels(true, &close.ticks), ["0", "10"]);
    assert!(close.ticks.iter().any(|tick| tick.rank == TickRank::Minor));

    let far = ruler(0.0, 0.1, 100.0).unwrap();
    near(far.step, 500.0);
    assert_eq!(labels(true, &far.ticks), ["0", "500", "1000"]);
}

#[test]
fn a_long_viewport_keeps_a_bounded_list_of_ticks() {
    let wide = ruler(0.0, 1.0, 1_000_000.0).unwrap();
    near(wide.step, 500.0);
    assert!(wide.ticks.len() <= 4096);
    assert!(wide.ticks.iter().all(|tick| tick.rank == TickRank::Major));
    assert!(wide.ticks.len() >= 2);
    near(wide.ticks[1].screen - wide.ticks[0].screen, 500.0);
}

#[test]
fn bad_numbers_are_refused() {
    assert_eq!(ruler(f64::NAN, 1.0, 10.0), Err(RulerError::NotFinite));
    assert_eq!(ruler(0.0, f64::INFINITY, 10.0), Err(RulerError::NotFinite));
    assert_eq!(ruler(0.0, 1.0, f64::NAN), Err(RulerError::NotFinite));
    assert_eq!(ruler(0.0, 0.0, 10.0), Err(RulerError::Zoom));
    assert_eq!(ruler(0.0, -1.0, 10.0), Err(RulerError::Zoom));
    assert_eq!(ruler(0.0, 1.0, -1.0), Err(RulerError::Length));
    assert!(screen_at(1.0, 0.0, 0.0).is_none());
    assert!(canvas_at(f64::NAN, 0.0, 1.0).is_none());
    let back = screen_at(canvas_at(40.0, 10.0, 2.0).unwrap(), 10.0, 2.0).unwrap();
    near(back, 40.0);
}

#[test]
fn a_frame_span_is_clipped_to_the_viewport() {
    let inside = span(0.0, 390.0, 10.0, 1.0, 500.0).unwrap().unwrap();
    near(inside.start, 10.0);
    near(inside.end, 400.0);
    let clipped = span(0.0, 390.0, -100.0, 1.0, 200.0).unwrap().unwrap();
    near(clipped.start, 0.0);
    near(clipped.end, 200.0);
    assert!(span(1000.0, 10.0, 0.0, 1.0, 100.0).unwrap().is_none());
    let point = span(10.0, 0.0, 0.0, 1.0, 100.0).unwrap().unwrap();
    near(point.start, 10.0);
    near(point.end, 10.0);
    assert_eq!(span(0.0, -1.0, 0.0, 1.0, 100.0), Err(RulerError::Extent));
    assert_eq!(
        span(f64::NAN, 10.0, 0.0, 1.0, 100.0),
        Err(RulerError::NotFinite)
    );
}

#[test]
fn the_readout_names_the_point_under_the_cursor() {
    assert_eq!(readout(10.4, 0.0, 1.0).unwrap(), "10.4");
    assert_eq!(readout(10.0, 0.0, 1.0).unwrap(), "10");
    assert_eq!(readout(0.0, 25.0, 1.0).unwrap(), "-25");
    assert_eq!(readout(f64::NAN, 0.0, 1.0), Err(RulerError::NotFinite));
}

#[test]
fn both_edges_share_the_zoom() {
    let both = rulers(0.0, 15.0, 1.0, 200.0, 80.0).unwrap();
    let horizontal = ruler(0.0, 1.0, 200.0).unwrap();
    let vertical = ruler(15.0, 1.0, 80.0).unwrap();
    assert_eq!(both.horizontal, horizontal);
    assert_eq!(both.vertical, vertical);
    near(both.horizontal.step, both.vertical.step);
}
