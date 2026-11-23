use windows::Graphics::Imaging::BitmapDecoder;
use windows::Media::Ocr::OcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

use crate::capture::CapturedFrame;
use crate::models::{DetectedText, DetectedWord, NormalizedRect};

pub struct OcrService;

impl OcrService {
    /// Runs Windows Media OCR on a captured window frame
    pub fn scan_frame(frame: &CapturedFrame) -> Vec<DetectedText> {
        if let Ok(results) = Self::scan_internal(frame) {
            results
        } else {
            Vec::new()
        }
    }

    fn scan_internal(frame: &CapturedFrame) -> windows::core::Result<Vec<DetectedText>> {
        // Encode frame to BMP in memory to pass to Windows BitmapDecoder
        let mut bmp_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut bmp_bytes);
        if let Some(img) = image::RgbaImage::from_raw(frame.width as u32, frame.height as u32, frame.rgba_data.clone()) {
            let _ = image::DynamicImage::ImageRgba8(img).write_to(&mut cursor, image::ImageFormat::Bmp);
        } else {
            return Ok(Vec::new());
        }

        let stream = InMemoryRandomAccessStream::new()?;
        let writer = DataWriter::CreateDataWriter(&stream)?;
        writer.WriteBytes(&bmp_bytes)?;
        writer.StoreAsync()?.get()?;
        writer.FlushAsync()?.get()?;
        stream.Seek(0)?;

        let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
        let bitmap = decoder.GetSoftwareBitmapAsync()?.get()?;

        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        let ocr_result = engine.RecognizeAsync(&bitmap)?.get()?;

        let mut detected_items = Vec::new();
        let img_w = frame.width as f32;
        let img_h = frame.height as f32;

        let lines = ocr_result.Lines()?;
        for line in lines {
            let text = line.Text()?.to_string();
            let words = line.Words()?;

            let mut word_boxes = Vec::new();
            for word in words {
                let word_text = word.Text()?.to_string();
                let rect = word.BoundingRect()?;

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
                let min_x = word_boxes.iter().map(|w| w.bounding_box.min_x).fold(1.0f32, f32::min);
                let min_y = word_boxes.iter().map(|w| w.bounding_box.min_y).fold(1.0f32, f32::min);
                let max_x = word_boxes.iter().map(|w| w.bounding_box.max_x).fold(0.0f32, f32::max);
                let max_y = word_boxes.iter().map(|w| w.bounding_box.max_y).fold(0.0f32, f32::max);

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

    /// Crops and 4x super-samples the left 20% of the screen for ultra-precise event text recognition
    pub fn scan_left_sidebar(frame: &CapturedFrame, sidebar_ratio: f32) -> Vec<DetectedText> {
        let sidebar_w = ((frame.width as f32) * sidebar_ratio).max(50.0) as u32;
        let sidebar_h = frame.height as u32;

        let Some(img) = image::RgbaImage::from_raw(frame.width as u32, frame.height as u32, frame.rgba_data.clone()) else {
            return Vec::new();
        };

        let cropped = image::imageops::crop_imm(&img, 0, 0, sidebar_w, sidebar_h).to_image();
        let upscaled = image::imageops::resize(&cropped, sidebar_w * 2, sidebar_h * 2, image::imageops::FilterType::CatmullRom);

        let sub_frame = CapturedFrame {
            width: (sidebar_w * 2) as i32,
            height: (sidebar_h * 2) as i32,
            rgba_data: upscaled.into_raw(),
        };

        let mut items = Self::scan_frame(&sub_frame);

        // Remap coordinates back to the full window coordinate space
        let width_factor = (sidebar_w as f32) / (frame.width as f32);
        for item in &mut items {
            item.bounding_box.min_x *= width_factor;
            item.bounding_box.max_x *= width_factor;
            for wb in &mut item.word_boxes {
                wb.bounding_box.min_x *= width_factor;
                wb.bounding_box.max_x *= width_factor;
            }
        }

        items
    }
}
