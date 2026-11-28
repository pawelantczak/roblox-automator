use crate::models::{CapturedFrame, DetectedText, NormalizedPoint, NormalizedRect};

pub struct IconTracer;

impl IconTracer {
    /// Traces the dark cartoon border (Roblox UIStroke, luminance < 48) of an icon graphic
    /// directly above the detected text bounding box.
    pub fn trace_icon_bounds(
        frame: &CapturedFrame,
        text_box: &NormalizedRect,
    ) -> Option<(NormalizedRect, NormalizedPoint)> {
        let img_w = frame.width as f32;
        let img_h = frame.height as f32;
        if img_w < 50.0 || img_h < 50.0 {
            return None;
        }

        // Convert normalized coordinates to pixel coordinates (top-down 0..height)
        let text_pixel_x = text_box.min_x * img_w;
        let text_pixel_w = text_box.width() * img_w;
        let text_pixel_top_y = text_box.min_y * img_h;
        let text_pixel_h = text_box.height() * img_h;
        let text_mid_x = text_pixel_x + (text_pixel_w * 0.5);

        // Define Region of Interest (ROI) directly above the text label (lower Y in top-down).
        // Use frame-relative minimums so tight single-line OCR boxes (e.g. 8-9px tall at 800x600)
        // still cover the full sidebar icon above the caption.
        let roi_w = (text_pixel_w * 1.4)
            .max(img_w * 0.08)
            .clamp(32.0, img_w * 0.25);
        let roi_h = (text_pixel_h * 3.5)
            .max(img_h * 0.11)
            .clamp(36.0, img_h * 0.22);
        let roi_x = (text_mid_x - (roi_w * 0.5)).max(0.0);
        let roi_y = (text_pixel_top_y - roi_h).max(0.0);
        let roi_actual_w = (img_w - roi_x).min(roi_w);
        let roi_actual_h = (text_pixel_top_y - roi_y).max(10.0);

        let roi_w_int = roi_actual_w as i32;
        let roi_h_int = roi_actual_h as i32;
        if roi_w_int < 15 || roi_h_int < 15 {
            return None;
        }

        let dark_cutoff = 48;
        let mut dark_points: Vec<(i32, i32)> = Vec::new();
        let mut col_dark_counts = vec![0i32; roi_w_int as usize];
        let mut row_dark_counts = vec![0i32; roi_h_int as usize];

        for local_y in 0..roi_h_int {
            let global_y = (roi_y as i32) + local_y;
            if global_y >= frame.height {
                break;
            }
            let row_offset = (global_y * frame.width * 4) as usize;

            for local_x in 0..roi_w_int {
                let global_x = (roi_x as i32) + local_x;
                if global_x >= frame.width {
                    break;
                }
                let pixel_offset = row_offset + (global_x * 4) as usize;

                let r = frame.rgba_data[pixel_offset] as i32;
                let g = frame.rgba_data[pixel_offset + 1] as i32;
                let b = frame.rgba_data[pixel_offset + 2] as i32;
                let lum = (r * 299 + g * 587 + b * 114) / 1000;

                if lum < dark_cutoff {
                    dark_points.push((local_x, local_y));
                    col_dark_counts[local_x as usize] += 1;
                    row_dark_counts[local_y as usize] += 1;
                }
            }
        }

        if dark_points.len() < 20 {
            return None;
        }

        // Bottom-up row-gap isolation: start just above the text and scan upward so we stop
        // before any button/text above the icon (e.g. "Ready!").
        let (min_row_y, max_row_y) = Self::isolate_bottom_up_row_bounds(&row_dark_counts, roi_h_int)?;

        // Recompute column counts restricted to the isolated vertical icon band
        let mut band_col_counts = vec![0i32; roi_w_int as usize];
        for &(x, y) in &dark_points {
            if y >= min_row_y && y <= max_row_y {
                band_col_counts[x as usize] += 1;
            }
        }

        // Column gap isolation
        let local_mid_x = ((text_mid_x - roi_x) as i32).clamp(0, roi_w_int - 1);
        let (min_col_x, max_col_x) = Self::isolate_column_bounds(&band_col_counts, local_mid_x, roi_w_int)?;

        let icon_dark: Vec<&(i32, i32)> = dark_points
            .iter()
            .filter(|(x, y)| *x >= min_col_x && *x <= max_col_x && *y >= min_row_y && *y <= max_row_y)
            .collect();

        if icon_dark.len() < 20 {
            return None;
        }

        let min_y = icon_dark.iter().map(|(_, y)| *y).min().unwrap_or(min_row_y);
        let max_y = icon_dark.iter().map(|(_, y)| *y).max().unwrap_or(max_row_y);

        let icon_pixel_w = (max_col_x - min_col_x) as f32;
        let icon_pixel_h = (max_y - min_y) as f32;
        if icon_pixel_w < 8.0 || icon_pixel_h < 8.0 {
            return None;
        }

        let abs_center_x = roi_x + ((min_col_x + max_col_x) as f32 * 0.5);
        let abs_center_y = roi_y + ((min_y + max_y) as f32 * 0.5);

        let norm_center = NormalizedPoint {
            x: abs_center_x / img_w,
            y: abs_center_y / img_h,
        };

        let norm_box = NormalizedRect::new(
            (roi_x + min_col_x as f32) / img_w,
            (roi_y + min_y as f32) / img_h,
            (roi_x + max_col_x as f32) / img_w,
            (roi_y + max_y as f32) / img_h,
        );

        Some((norm_box, norm_center))
    }

    /// Scans upward from the bottom of the ROI (just above the text label) to find the first
    /// vertical icon band, stopping at a 2-row empty gap so elements above the icon are excluded.
    fn isolate_bottom_up_row_bounds(row_counts: &[i32], height: i32) -> Option<(i32, i32)> {
        let mut max_y = height - 1;
        while max_y > 0 && row_counts[max_y as usize] == 0 {
            max_y -= 1;
        }
        if row_counts[max_y as usize] == 0 {
            return None;
        }
        let mut min_y = max_y;
        while min_y > 0 {
            if row_counts[min_y as usize] == 0 && row_counts[(min_y - 1).max(0) as usize] == 0 {
                break;
            }
            min_y -= 1;
        }
        while min_y < max_y && row_counts[min_y as usize] == 0 {
            min_y += 1;
        }
        if max_y - min_y < 8 {
            return None;
        }
        Some((min_y, max_y))
    }

    /// Isolates horizontal bounds of the central icon by scanning left/right from text center
    /// until hitting zero-dark-pixel column gaps that separate adjacent UI buttons.
    fn isolate_column_bounds(col_counts: &[i32], local_mid_x: i32, width: i32) -> Option<(i32, i32)> {
        let mut left_x = local_mid_x;
        while left_x > 0 {
            if col_counts[left_x as usize] == 0 && col_counts[(left_x - 1).max(0) as usize] == 0 {
                break;
            }
            left_x -= 1;
        }
        while left_x < local_mid_x && col_counts[left_x as usize] == 0 {
            left_x += 1;
        }

        let mut right_x = local_mid_x;
        while right_x < width - 1 {
            if col_counts[right_x as usize] == 0 && col_counts[(right_x + 1).min(width - 1) as usize] == 0 {
                break;
            }
            right_x += 1;
        }
        while right_x > local_mid_x && col_counts[right_x as usize] == 0 {
            right_x -= 1;
        }

        if right_x - left_x < 8 {
            return None;
        }

        Some((left_x, right_x))
    }

    /// Refines target coordinates by tracing cartoon stroke contour
    pub fn refine_target(target: DetectedText, frame: &CapturedFrame) -> DetectedText {
        if let Some((traced_box, traced_center)) = Self::trace_icon_bounds(frame, &target.bounding_box) {
            DetectedText {
                text: target.text,
                bounding_box: target.bounding_box,
                target_point: Some(traced_center),
                whole_icon_box: Some(traced_box),
                word_boxes: target.word_boxes,
            }
        } else {
            target
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn white_frame(w: i32, h: i32) -> CapturedFrame {
        CapturedFrame::new(w, h, vec![255u8; (w * h * 4) as usize])
    }

    fn draw_dark_box(frame: &mut CapturedFrame, x0: i32, y0: i32, x1: i32, y1: i32) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                if x == x0 || x == x1 || y == y0 || y == y1 {
                    let idx = ((y * frame.width + x) * 4) as usize;
                    frame.rgba_data[idx] = 10;
                    frame.rgba_data[idx + 1] = 10;
                    frame.rgba_data[idx + 2] = 10;
                    frame.rgba_data[idx + 3] = 255;
                }
            }
        }
    }

    #[test]
    fn test_trace_icon_bounds_finds_dark_outline_above_tight_text() {
        let mut frame = white_frame(300, 300);
        // 40x40 cartoon outline icon at x=130..170, y=140..180
        draw_dark_box(&mut frame, 130, 140, 170, 180);
        // Tight 9px caption directly below at x=125..175, y=190..199
        let text_box = NormalizedRect::new(125.0 / 300.0, 190.0 / 300.0, 175.0 / 300.0, 199.0 / 300.0);

        let (traced_box, center) =
            IconTracer::trace_icon_bounds(&frame, &text_box).expect("should trace dark outline");
        assert!((center.x - 0.5).abs() < 0.03, "expected center.x ~ 0.5, got {}", center.x);
        assert!((center.y - (160.0 / 300.0)).abs() < 0.03, "expected center.y ~ 0.533, got {}", center.y);
        assert!(traced_box.min_y < text_box.min_y);
    }

    #[test]
    fn test_trace_icon_bounds_isolates_from_upper_and_side_noise() {
        let mut frame = white_frame(300, 300);
        // Target icon
        draw_dark_box(&mut frame, 130, 150, 170, 185);
        // Upper unrelated button separated by gap
        draw_dark_box(&mut frame, 130, 100, 170, 120);
        // Side unrelated icon separated by column gap
        draw_dark_box(&mut frame, 50, 150, 80, 185);

        let text_box = NormalizedRect::new(125.0 / 300.0, 192.0 / 300.0, 175.0 / 300.0, 205.0 / 300.0);
        let (traced_box, _) =
            IconTracer::trace_icon_bounds(&frame, &text_box).expect("should isolate central icon");
        assert!(traced_box.min_y >= 145.0 / 300.0, "should stop below upper button");
        assert!(traced_box.min_x >= 120.0 / 300.0, "should stop right of side icon");
    }

    #[test]
    fn test_refine_target_falls_back_when_no_outline() {
        let frame = white_frame(200, 200);
        let orig = DetectedText {
            text: "Event".to_string(),
            bounding_box: NormalizedRect::new(0.1, 0.5, 0.2, 0.55),
            target_point: Some(NormalizedPoint { x: 0.15, y: 0.45 }),
            whole_icon_box: None,
            word_boxes: vec![],
        };
        let refined = IconTracer::refine_target(orig.clone(), &frame);
        assert_eq!(refined, orig);
    }
}
