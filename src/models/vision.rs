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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalized_rect_geometry_and_click_point() {
        let r = NormalizedRect::new(0.2, 0.4, 0.6, 0.8);
        assert!((r.mid_x() - 0.4).abs() < 1e-6);
        assert!((r.mid_y() - 0.6).abs() < 1e-6);
        assert!((r.width() - 0.4).abs() < 1e-6);
        assert!((r.height() - 0.4).abs() < 1e-6);

        let fallback = DetectedText {
            text: "Btn".into(),
            bounding_box: r.clone(),
            target_point: None,
            whole_icon_box: None,
            word_boxes: vec![],
        };
        assert_eq!(fallback.click_point(), NormalizedPoint { x: 0.4, y: 0.6 });

        let explicit = DetectedText {
            target_point: Some(NormalizedPoint { x: 0.11, y: 0.22 }),
            ..fallback
        };
        assert_eq!(explicit.click_point(), NormalizedPoint { x: 0.11, y: 0.22 });
    }

    #[test]
    fn test_captured_frame_sampled_hash_detects_changes() {
        let f1 = CapturedFrame::new(64, 64, vec![10u8; 64 * 64 * 4]);
        let f2 = CapturedFrame::new(64, 64, vec![10u8; 64 * 64 * 4]);
        assert_eq!(f1.frame_hash, f2.frame_hash);

        let mut modified = vec![10u8; 64 * 64 * 4];
        modified[64] = 250;
        let f3 = CapturedFrame::new(64, 64, modified);
        assert_ne!(f1.frame_hash, f3.frame_hash);
    }
}
