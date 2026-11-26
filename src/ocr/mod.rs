use std::time::{Duration, Instant};

use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

use crate::models::{CapturedFrame, DetectedText, DetectedWord, NormalizedRect};

pub trait OcrProvider: Send + Sync {
    fn scan_frame(&self, frame: &CapturedFrame, timeout: Duration) -> Result<Vec<DetectedText>, String>;
    fn scan_left_sidebar(
        &self,
        frame: &CapturedFrame,
        sidebar_ratio: f32,
        timeout: Duration,
    ) -> Result<Vec<DetectedText>, String>;
    fn scan_left_sidebar_high_contrast(
        &self,
        frame: &CapturedFrame,
        sidebar_ratio: f32,
        timeout: Duration,
    ) -> Result<Vec<DetectedText>, String>;
    fn scan_region_upscaled(
        &self,
        _frame: &CapturedFrame,
        _region: NormalizedRect,
        _timeout: Duration,
    ) -> Result<Vec<DetectedText>, String> {
        Ok(Vec::new())
    }
}

pub struct WindowsMediaOcrService;

impl WindowsMediaOcrService {
    fn scan_internal(frame: &CapturedFrame, timeout: Duration) -> Result<Vec<DetectedText>, String> {
        let start = Instant::now();

        let mut bmp_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut bmp_bytes);
        let Some(img) = image::RgbaImage::from_raw(
            frame.width as u32,
            frame.height as u32,
            frame.rgba_data.clone(),
        ) else {
            return Ok(Vec::new());
        };

        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut cursor, image::ImageFormat::Bmp)
            .map_err(|e| format!("BMP encode error: {}", e))?;

        if start.elapsed() > timeout {
            return Err("OCR timed out before decoding".to_string());
        }

        let stream = InMemoryRandomAccessStream::new().map_err(|e| e.to_string())?;
        let writer = DataWriter::CreateDataWriter(&stream).map_err(|e| e.to_string())?;
        writer.WriteBytes(&bmp_bytes).map_err(|e| e.to_string())?;
        writer.StoreAsync().map_err(|e| e.to_string())?.get().map_err(|e| e.to_string())?;
        writer.FlushAsync().map_err(|e| e.to_string())?.get().map_err(|e| e.to_string())?;
        stream.Seek(0).map_err(|e| e.to_string())?;

        let decoder = BitmapDecoder::CreateAsync(&stream)
            .map_err(|e| e.to_string())?
            .get()
            .map_err(|e| e.to_string())?;
        let bitmap = decoder
            .GetSoftwareBitmapAsync()
            .map_err(|e| e.to_string())?
            .get()
            .map_err(|e| e.to_string())?;

        if start.elapsed() > timeout {
            return Err("OCR timed out before recognition".to_string());
        }

        let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|e| e.to_string())?;
        let ocr_result = engine
            .RecognizeAsync(&bitmap)
            .map_err(|e| e.to_string())?
            .get()
            .map_err(|e| e.to_string())?;

        if start.elapsed() > timeout {
            return Err("OCR exceeded configured timeout".to_string());
        }

        let mut detected_items = Vec::new();
        let img_w = frame.width as f32;
        let img_h = frame.height as f32;

        let lines = ocr_result.Lines().map_err(|e| e.to_string())?;
        for line in lines {
            let text = line.Text().map_err(|e| e.to_string())?.to_string();
            let words = line.Words().map_err(|e| e.to_string())?;

            let mut word_boxes = Vec::new();
            for word in words {
                let word_text = word.Text().map_err(|e| e.to_string())?.to_string();
                let rect = word.BoundingRect().map_err(|e| e.to_string())?;

                let min_x = rect.X / img_w;
                let min_y = rect.Y / img_h;
                let max_x = (rect.X + rect.Width) / img_w;
                let max_y = (rect.Y + rect.Height) / img_h;

                word_boxes.push(DetectedWord {
                    word: word_text,
                    bounding_box: NormalizedRect::new(min_x, min_y, max_x, max_y),
                });
            }

            if !word_boxes.is_empty() {
                let min_x = word_boxes
                    .iter()
                    .map(|w| w.bounding_box.min_x)
                    .fold(1.0f32, f32::min);
                let min_y = word_boxes
                    .iter()
                    .map(|w| w.bounding_box.min_y)
                    .fold(1.0f32, f32::min);
                let max_x = word_boxes
                    .iter()
                    .map(|w| w.bounding_box.max_x)
                    .fold(0.0f32, f32::max);
                let max_y = word_boxes
                    .iter()
                    .map(|w| w.bounding_box.max_y)
                    .fold(0.0f32, f32::max);

                detected_items.push(DetectedText {
                    text,
                    bounding_box: NormalizedRect::new(min_x, min_y, max_x, max_y),
                    target_point: None,
                    whole_icon_box: None,
                    word_boxes,
                });
            }
        }

        Ok(detected_items)
    }
}

impl OcrProvider for WindowsMediaOcrService {
    fn scan_frame(&self, frame: &CapturedFrame, timeout: Duration) -> Result<Vec<DetectedText>, String> {
        Self::scan_internal(frame, timeout)
    }

    fn scan_left_sidebar(
        &self,
        frame: &CapturedFrame,
        sidebar_ratio: f32,
        timeout: Duration,
    ) -> Result<Vec<DetectedText>, String> {
        let sidebar_w = ((frame.width as f32) * sidebar_ratio).max(50.0) as u32;
        let sidebar_h = frame.height as u32;

        let Some(img) = image::RgbaImage::from_raw(
            frame.width as u32,
            frame.height as u32,
            frame.rgba_data.clone(),
        ) else {
            return Ok(Vec::new());
        };

        let cropped = image::imageops::crop_imm(&img, 0, 0, sidebar_w, sidebar_h).to_image();
        let max_dim = sidebar_w.max(sidebar_h).max(1) as f32;
        let scale = (2200.0 / max_dim).clamp(1.0, 2.0);
        let up_w = ((sidebar_w as f32) * scale).round() as u32;
        let up_h = ((sidebar_h as f32) * scale).round() as u32;

        let upscaled = image::imageops::resize(
            &cropped,
            up_w,
            up_h,
            image::imageops::FilterType::CatmullRom,
        );

        let sub_frame = CapturedFrame::new(up_w as i32, up_h as i32, upscaled.into_raw());

        let mut items = Self::scan_internal(&sub_frame, timeout)?;

        let width_factor = (sidebar_w as f32) / (frame.width as f32);
        for item in &mut items {
            item.bounding_box.min_x *= width_factor;
            item.bounding_box.max_x *= width_factor;
            for wb in &mut item.word_boxes {
                wb.bounding_box.min_x *= width_factor;
                wb.bounding_box.max_x *= width_factor;
            }
        }

        Ok(items)
    }

    /// 3x upscaled high-contrast pass isolating bright white Roblox UI text as black-on-white
    /// to eliminate noisy 3D background interference.
    fn scan_left_sidebar_high_contrast(
        &self,
        frame: &CapturedFrame,
        sidebar_ratio: f32,
        timeout: Duration,
    ) -> Result<Vec<DetectedText>, String> {
        let sidebar_w = ((frame.width as f32) * sidebar_ratio).max(50.0) as u32;
        let sidebar_h = frame.height as u32;

        let Some(img) = image::RgbaImage::from_raw(
            frame.width as u32,
            frame.height as u32,
            frame.rgba_data.clone(),
        ) else {
            return Ok(Vec::new());
        };

        let cropped = image::imageops::crop_imm(&img, 0, 0, sidebar_w, sidebar_h).to_image();
        let max_dim = sidebar_w.max(sidebar_h).max(1) as f32;
        let scale = (2200.0 / max_dim).clamp(1.0, 3.0);
        let up_w = ((sidebar_w as f32) * scale).round() as u32;
        let up_h = ((sidebar_h as f32) * scale).round() as u32;

        let mut upscaled = image::imageops::resize(
            &cropped,
            up_w,
            up_h,
            image::imageops::FilterType::CatmullRom,
        );

        for pixel in upscaled.pixels_mut() {
            let [r, g, b, _] = pixel.0;
            let lum = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
            if lum >= 180 {
                // Bright UI text -> crisp black ink
                pixel.0 = [0, 0, 0, 255];
            } else {
                // Colorful background -> pure white paper
                pixel.0 = [255, 255, 255, 255];
            }
        }

        let sub_frame = CapturedFrame::new(up_w as i32, up_h as i32, upscaled.into_raw());

        let mut items = Self::scan_internal(&sub_frame, timeout)?;

        let width_factor = (sidebar_w as f32) / (frame.width as f32);
        for item in &mut items {
            item.bounding_box.min_x *= width_factor;
            item.bounding_box.max_x *= width_factor;
            for wb in &mut item.word_boxes {
                wb.bounding_box.min_x *= width_factor;
                wb.bounding_box.max_x *= width_factor;
            }
        }

        Ok(items)
    }

    fn scan_region_upscaled(
        &self,
        frame: &CapturedFrame,
        region: NormalizedRect,
        timeout: Duration,
    ) -> Result<Vec<DetectedText>, String> {
        let min_x = region.min_x.clamp(0.0, 0.95);
        let min_y = region.min_y.clamp(0.0, 0.95);
        let max_x = region.max_x.clamp(min_x + 0.05, 1.0);
        let max_y = region.max_y.clamp(min_y + 0.05, 1.0);

        let fw = frame.width.max(1) as f32;
        let fh = frame.height.max(1) as f32;
        let crop_x = (min_x * fw).round() as u32;
        let crop_y = (min_y * fh).round() as u32;
        let crop_w = ((max_x - min_x) * fw).round().max(32.0) as u32;
        let crop_h = ((max_y - min_y) * fh).round().max(32.0) as u32;

        let Some(img) = image::RgbaImage::from_raw(
            frame.width as u32,
            frame.height as u32,
            frame.rgba_data.clone(),
        ) else {
            return Ok(Vec::new());
        };

        let crop_w = crop_w.min(img.width().saturating_sub(crop_x)).max(1);
        let crop_h = crop_h.min(img.height().saturating_sub(crop_y)).max(1);
        let cropped = image::imageops::crop_imm(&img, crop_x, crop_y, crop_w, crop_h).to_image();

        let max_dim = crop_w.max(crop_h).max(1) as f32;
        let scale = (2200.0 / max_dim).clamp(1.5, 2.5);
        let up_w = ((crop_w as f32) * scale).round() as u32;
        let up_h = ((crop_h as f32) * scale).round() as u32;

        let upscaled = image::imageops::resize(
            &cropped,
            up_w,
            up_h,
            image::imageops::FilterType::CatmullRom,
        );

        let sub_frame = CapturedFrame::new(up_w as i32, up_h as i32, upscaled.into_raw());
        let mut items = Self::scan_internal(&sub_frame, timeout)?;

        let span_x = max_x - min_x;
        let span_y = max_y - min_y;
        let map_rect = |r: &mut NormalizedRect| {
            r.min_x = min_x + r.min_x * span_x;
            r.max_x = min_x + r.max_x * span_x;
            r.min_y = min_y + r.min_y * span_y;
            r.max_y = min_y + r.max_y * span_y;
        };

        for item in &mut items {
            map_rect(&mut item.bounding_box);
            for wb in &mut item.word_boxes {
                map_rect(&mut wb.bounding_box);
            }
        }

        Ok(items)
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use crate::engine::classifier::Classifier;
    use crate::models::Config;
    use std::path::Path;

    #[test]
    fn test_ocr_on_user_uploaded_images() {
        let paths = [
            (
                r"C:\Users\pawel\.gemini\antigravity\brain\f6e91f92-803e-4bb8-88a4-600c11ea779b\.user_uploaded\media_1795543667572_d18f21dd.jpg",
                "Retry",
            ),
            (
                r"C:\Users\pawel\.gemini\antigravity\brain\f6e91f92-803e-4bb8-88a4-600c11ea779b\.user_uploaded\media_1795543699605_91b87493.jpg",
                "Połącz ponownie",
            ),
        ];

        let config = Config::default();
        let ocr = WindowsMediaOcrService;

        for (idx, (p, expected_button)) in paths.iter().enumerate() {
            if !Path::new(p).exists() {
                continue;
            }
            let img = image::open(p).expect("failed to open image").to_rgba8();
            let frame = CapturedFrame::new(img.width() as i32, img.height() as i32, img.into_raw());

            let detected = ocr
                .scan_frame(&frame, Duration::from_secs(5))
                .expect("OCR failed");
            assert!(!detected.is_empty(), "Image {} should detect text items", idx + 1);

            let has_disconnect = detected.iter().any(|item| {
                Classifier::is_fatal_disconnect(
                    &item.text,
                    &config.disconnect_keywords,
                    &config.ignore_phrases,
                )
            });
            assert!(has_disconnect, "Image {} should trigger fatal disconnect", idx + 1);

            let btn = Classifier::find_reconnect_button(
                &detected,
                &config.reconnect_button_phrases,
                &config.ignore_phrases,
            );
            assert!(
                btn.is_some(),
                "Image {} should find reconnect button '{}'",
                idx + 1,
                expected_button
            );
            assert!(btn
                .unwrap()
                .text
                .to_lowercase()
                .contains(&expected_button.to_lowercase()));
        }
    }
}
