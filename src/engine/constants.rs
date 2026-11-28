/// Fraction of window width scanned for left-sidebar event icons.
pub const SIDEBAR_OCR_WIDTH_RATIO: f32 = 0.35;

/// Maximum normalized X coordinate considered part of the left sidebar.
pub const EVENT_MAX_NORMALIZED_X: f32 = 0.40;

/// Normalized center-modal region used for upscaled reconnect button fallback OCR.
pub const MODAL_OCR_REGION: (f32, f32, f32, f32) = (0.15, 0.15, 0.85, 0.90);

/// Luminance cutoff for detecting dark Roblox UIStroke borders around cartoon icons.
pub const DARK_STROKE_LUMA_CUTOFF: i32 = 48;
