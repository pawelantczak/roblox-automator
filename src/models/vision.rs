#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl NormalizedRect {
    pub fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    pub fn mid_x(&self) -> f32 {
        (self.min_x + self.max_x) * 0.5
    }

    pub fn mid_y(&self) -> f32 {
        (self.min_y + self.max_y) * 0.5
    }

    pub fn width(&self) -> f32 {
        (self.max_x - self.min_x).abs()
    }

    pub fn height(&self) -> f32 {
        (self.max_y - self.min_y).abs()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedWord {
    pub word: String,
    pub bounding_box: NormalizedRect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedText {
    pub text: String,
    pub bounding_box: NormalizedRect,
    pub target_point: Option<NormalizedPoint>,
    pub whole_icon_box: Option<NormalizedRect>,
    pub word_boxes: Vec<DetectedWord>,
}

impl DetectedText {
    pub fn click_point(&self) -> NormalizedPoint {
        if let Some(ref pt) = self.target_point {
            pt.clone()
        } else {
            NormalizedPoint {
                x: self.bounding_box.mid_x(),
                y: self.bounding_box.mid_y(),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub width: i32,
    pub height: i32,
    pub rgba_data: Vec<u8>,
    pub frame_hash: u64,
}

impl CapturedFrame {
    pub fn new(width: i32, height: i32, rgba_data: Vec<u8>) -> Self {
        let frame_hash = Self::compute_sampled_hash(&rgba_data);
        Self {
            width,
            height,
            rgba_data,
            frame_hash,
        }
    }

    /// Fast sampled FNV-1a hash over pixel strides for cheap frame-difference detection.
    pub fn compute_sampled_hash(rgba: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        // Sample every 64th byte (16th pixel) for sub-millisecond frame diffing
        for &b in rgba.iter().step_by(64) {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}
