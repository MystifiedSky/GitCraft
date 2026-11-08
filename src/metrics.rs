//! Layout and behaviour values read from the original (see MECHANICS.md / mechanics.json).
//! The front end asks for these at start-up and draws the graph and commit box with them.
//! Graph values come from the graph constants module in render.bundle.js
//! (exported names such as GRAPH_ROW_HEIGHT, COMMIT_ZONE_EDGE_ARC_RADIUS).

use serde::Serialize;

/// COMMIT_NODE_DIAMETER
pub const COMMIT_NODE_DIAMETER: u32 = 22;
/// COMMIT_AUTHOR_ZONE_AVATAR_INNER_DIAMETER (avatar inside the node ring)
pub const AVATAR_INNER_DIAMETER: u32 = 18;
/// COMMIT_COLUMN_WIDTH (one graph lane)
pub const GRAPH_LANE_WIDTH: u32 = 22;
/// COMMIT_ZONE_GUTTER_WIDTH
pub const GRAPH_GUTTER_WIDTH: u32 = 28;
/// COMMIT_ZONE_PADDING_LEFT / _RIGHT
pub const GRAPH_ZONE_PADDING: u32 = 3;
/// COMMIT_ZONE_DEFAULT_WIDTH
pub const GRAPH_ZONE_DEFAULT_WIDTH: u32 = 150;
/// REF_ZONE_DEFAULT_WIDTH
pub const REF_ZONE_DEFAULT_WIDTH: u32 = 130;
/// REF_ZONE_MIN_WIDTH
pub const REF_ZONE_MIN_WIDTH: u32 = 32;
/// REF_ZONE_MARGIN_LEFT + LeftPanelToGraphMarginGap (2 + 7)
pub const REF_ZONE_MARGIN_LEFT: u32 = 2;
pub const LEFT_PANEL_TO_GRAPH_GAP: u32 = 7;
/// REF_ZONE_MARGIN_RIGHT
pub const REF_ZONE_MARGIN_RIGHT: u32 = 6;
/// REF_ZONE_MAX_REFS_TO_RENDER
pub const REF_ZONE_MAX_REFS: u32 = 100;
/// COMMIT_MESSAGE_ZONE_DEFAULT_WIDTH
pub const MESSAGE_ZONE_DEFAULT_WIDTH: u32 = 300;
/// COMMIT_MESSAGE_ZONE_MIN_WIDTH
pub const MESSAGE_ZONE_MIN_WIDTH: u32 = 50;
/// INLINE_SUMMARY_MARGIN_LEFT
pub const INLINE_SUMMARY_MARGIN_LEFT: u32 = 10;
/// GRAPH_HEADER_ROW_HEIGHT
pub const GRAPH_HEADER_ROW_HEIGHT: u32 = 26;
/// GRAPH_ROW_HEIGHT
pub const GRAPH_ROW_HEIGHT: u32 = 28;
/// GRAPH_ROW_INNER_HEIGHT (message cell and branch label height inside a row)
pub const GRAPH_ROW_INNER_HEIGHT: u32 = 22;
/// COMMIT_ZONE_EDGE_ARC_RADIUS (rounded corner where a line turns to another lane)
pub const EDGE_ARC_RADIUS: u32 = 11;
/// COMMIT_NODE_MIN_ALPHA, in percent (faded commits)
pub const NODE_MIN_ALPHA_PERCENT: u32 = 50;
/// DEFAULT_COMMIT_SHA_LENGTH
pub const SHORT_SHA_LENGTH: u32 = 6;
/// DEFAULT_AVATAR_BACKGROUND_COLOR
pub const DEFAULT_AVATAR_BACKGROUND: &str = "#199489";
/// Characters the commit summary counter counts down from.
pub const COMMIT_SUMMARY_COUNTER: u32 = 72;
/// Auto-fetch interval default in minutes (used from Stage 4).
pub const AUTO_FETCH_INTERVAL_MIN: u32 = 1;
/// ZOOM_FACTORS (main.bundle.js / render.bundle.js): zoom in / out moves to the next entry.
pub const ZOOM_FACTORS: [f32; 12] = [3.0, 2.5, 2.0, 1.75, 1.5, 1.4, 1.3, 1.2, 1.1, 1.0, 0.9, 0.8];
/// DEFAULT_ZOOM_FACTOR
pub const DEFAULT_ZOOM_FACTOR: f32 = 1.0;

/// Working-folder watcher debounce: render.bundle.js passes {debounceMS:2e3} to nsfw.
pub const WORKDIR_WATCH_DEBOUNCE_MS: u32 = 2000;
/// .git-folder watcher debounce: render.bundle.js passes {debounceMS:200} to nsfw.
pub const GITDIR_WATCH_DEBOUNCE_MS: u32 = 200;
/// nsfw.node 0x1800030b0: "options.debounceMS must be >= 1 and <= 60000."
pub const WATCH_DEBOUNCE_MIN_MS: u32 = 1;
pub const WATCH_DEBOUNCE_MAX_MS: u32 = 60000;

#[derive(Serialize, Clone)]
pub struct Metrics {
    pub node: u32,
    pub avatar_inner: u32,
    pub lane: u32,
    pub gutter: u32,
    pub pad: u32,
    pub graph_zone: u32,
    pub ref_zone: u32,
    pub ref_zone_min: u32,
    pub ref_margin_left: u32,
    pub ref_margin_right: u32,
    pub ref_max: u32,
    pub message_zone: u32,
    pub message_min: u32,
    pub summary_margin: u32,
    pub header: u32,
    pub row: u32,
    pub row_inner: u32,
    pub arc_radius: u32,
    pub min_alpha: u32,
    pub sha_len: u32,
    pub avatar_bg: String,
    pub summary_counter: u32,
    pub auto_fetch_min: u32,
    pub zoom_factors: Vec<f32>,
    pub default_zoom: f32,
}

pub fn metrics() -> Metrics {
    Metrics {
        node: COMMIT_NODE_DIAMETER,
        avatar_inner: AVATAR_INNER_DIAMETER,
        lane: GRAPH_LANE_WIDTH,
        gutter: GRAPH_GUTTER_WIDTH,
        pad: GRAPH_ZONE_PADDING,
        graph_zone: GRAPH_ZONE_DEFAULT_WIDTH,
        ref_zone: REF_ZONE_DEFAULT_WIDTH,
        ref_zone_min: REF_ZONE_MIN_WIDTH,
        ref_margin_left: REF_ZONE_MARGIN_LEFT + LEFT_PANEL_TO_GRAPH_GAP,
        ref_margin_right: REF_ZONE_MARGIN_RIGHT,
        ref_max: REF_ZONE_MAX_REFS,
        message_zone: MESSAGE_ZONE_DEFAULT_WIDTH,
        message_min: MESSAGE_ZONE_MIN_WIDTH,
        summary_margin: INLINE_SUMMARY_MARGIN_LEFT,
        header: GRAPH_HEADER_ROW_HEIGHT,
        row: GRAPH_ROW_HEIGHT,
        row_inner: GRAPH_ROW_INNER_HEIGHT,
        arc_radius: EDGE_ARC_RADIUS,
        min_alpha: NODE_MIN_ALPHA_PERCENT,
        sha_len: SHORT_SHA_LENGTH,
        avatar_bg: DEFAULT_AVATAR_BACKGROUND.to_string(),
        summary_counter: COMMIT_SUMMARY_COUNTER,
        auto_fetch_min: AUTO_FETCH_INTERVAL_MIN,
        zoom_factors: ZOOM_FACTORS.to_vec(),
        default_zoom: DEFAULT_ZOOM_FACTOR,
    }
}
