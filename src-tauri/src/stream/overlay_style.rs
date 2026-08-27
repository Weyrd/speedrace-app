pub(super) struct OverlayStyle {
    pub max_splits: usize,
    pub margin: f32,
    pub pad: f32,
    pub timer_font: f32,
    pub split_font: f32,
    pub row_gap: f32,
    pub card_color: &'static str,
    pub card_alpha: f32,
    pub timer_color: &'static str,
    pub split_color: &'static str,
    pub split_alpha: f32,
    pub name_max_chars: usize,
}

pub(super) const DEFAULT_STYLE: OverlayStyle = OverlayStyle {
    max_splits: 3,
    margin: 0.015,
    pad: 0.008,
    timer_font: 0.025,
    split_font: 0.020,
    row_gap: 0.006,
    card_color: "black",
    card_alpha: 0.45,
    timer_color: "white",
    split_color: "white",
    split_alpha: 0.9,
    name_max_chars: 20,
};

pub(super) const MIN_FONT_PX: u32 = 12;
pub(super) const FONT_FILE: &str = "FiraMonoNerdFont-Regular.otf";
