use base64::prelude::*;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{ImageBuffer, Rgba};
use std::io::Cursor;
use windows::Win32::Foundation::{BOOL, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    GetWindowDC, HDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

use crate::models::{CapturedFrame, NormalizedPoint};

#[link(name = "user32")]
extern "system" {
    fn PrintWindow(hwnd: HWND, hdc_blt: HDC, n_flags: u32) -> BOOL;
}

pub trait ScreenCapture: Send + Sync {
    fn capture_window(&self, hwnd: HWND) -> Result<CapturedFrame, String>;
    fn encode_thumbnail_base64(
        &self,
        frame: &CapturedFrame,
        max_w: u32,
        max_h: u32,
        quality: u8,
    ) -> Option<String>;
    fn encode_point_crop_base64(
        &self,
        frame: &CapturedFrame,
        center: &NormalizedPoint,
        radius_px: u32,
        quality: u8,
    ) -> Option<String>;
}

pub struct Win32CaptureService;

impl ScreenCapture for Win32CaptureService {
    fn capture_window(&self, hwnd: HWND) -> Result<CapturedFrame, String> {
        unsafe {
            let mut win_rect = RECT::default();
            GetWindowRect(hwnd, &mut win_rect)
                .map_err(|e| format!("GetWindowRect failed: {}", e))?;

            let width = win_rect.right - win_rect.left;
            let height = win_rect.bottom - win_rect.top;

            if width <= 0 || height <= 0 {
                return Err("Window has zero or negative dimensions".to_string());
            }

            let hdc_screen = GetDC(HWND(std::ptr::null_mut()));
            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
            let old_bitmap = SelectObject(hdc_mem, hbitmap);

            // PW_RENDERFULLCONTENT = 2: renders entire window (including title bar & DirectX backbuffer)
            let pw_result = PrintWindow(hwnd, hdc_mem, 2);

            if !pw_result.as_bool() {
                let hdc_win = GetWindowDC(hwnd);
                let _ = BitBlt(hdc_mem, 0, 0, width, height, hdc_win, 0, 0, SRCCOPY);
                ReleaseDC(hwnd, hdc_win);
            }

            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD::default()],
            };

            let mut bgra_buf = vec![0u8; (width * height * 4) as usize];
            let lines = GetDIBits(
                hdc_mem,
                hbitmap,
                0,
                height as u32,
                Some(bgra_buf.as_mut_ptr() as *mut _),
                &mut bmi,
                DIB_RGB_COLORS,
            );

            SelectObject(hdc_mem, old_bitmap);
            let _ = DeleteObject(hbitmap);
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(HWND(std::ptr::null_mut()), hdc_screen);

            if lines <= 0 {
                return Err("GetDIBits returned 0 scanlines".to_string());
            }

            let mut rgba_data = bgra_buf;
            for chunk in rgba_data.chunks_exact_mut(4) {
                let b = chunk[0];
                let r = chunk[2];
                chunk[0] = r;
                chunk[2] = b;
                chunk[3] = 255;
            }

            Ok(CapturedFrame::new(width, height, rgba_data))
        }
    }

    fn encode_thumbnail_base64(
        &self,
        frame: &CapturedFrame,
        max_w: u32,
        max_h: u32,
        quality: u8,
    ) -> Option<String> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(frame.width as u32, frame.height as u32, frame.rgba_data.clone())?;

        // DynamicImage::resize preserves aspect ratio while fitting within max_w x max_h
        let resized = image::DynamicImage::ImageRgba8(img)
            .resize(max_w, max_h, FilterType::Triangle)
            .to_rgb8();

        let mut jpeg_bytes = Vec::new();
        let mut cursor = Cursor::new(&mut jpeg_bytes);
        let encoder = JpegEncoder::new_with_quality(&mut cursor, quality.clamp(30, 95));
        resized.write_with_encoder(encoder).ok()?;

        Some(BASE64_STANDARD.encode(jpeg_bytes))
    }

    fn encode_point_crop_base64(
        &self,
        frame: &CapturedFrame,
        center: &NormalizedPoint,
        radius_px: u32,
        quality: u8,
    ) -> Option<String> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(frame.width as u32, frame.height as u32, frame.rgba_data.clone())?;

        let fw = frame.width as u32;
        let fh = frame.height as u32;
        if fw == 0 || fh == 0 {
            return None;
        }

        let cx = ((center.x * fw as f32).round() as i32).clamp(0, fw as i32 - 1) as u32;
        let cy = ((center.y * fh as f32).round() as i32).clamp(0, fh as i32 - 1) as u32;

        let x0 = cx.saturating_sub(radius_px);
        let y0 = cy.saturating_sub(radius_px);
        let x1 = (cx + radius_px).min(fw);
        let y1 = (cy + radius_px).min(fh);

        let w = x1.saturating_sub(x0).max(1);
        let h = y1.saturating_sub(y0).max(1);

        let mut cropped = image::imageops::crop_imm(&img, x0, y0, w, h).to_image();

        // Draw a small cyan crosshair at the exact target point inside the crop
        let local_cx = cx.saturating_sub(x0) as i32;
        let local_cy = cy.saturating_sub(y0) as i32;
        for d in -4i32..=4 {
            let px = local_cx + d;
            if px >= 0 && (px as u32) < w && local_cy >= 0 && (local_cy as u32) < h {
                cropped.put_pixel(px as u32, local_cy as u32, Rgba([56, 189, 248, 255]));
            }
            let py = local_cy + d;
            if py >= 0 && (py as u32) < h && local_cx >= 0 && (local_cx as u32) < w {
                cropped.put_pixel(local_cx as u32, py as u32, Rgba([56, 189, 248, 255]));
            }
        }

        let rgb = image::DynamicImage::ImageRgba8(cropped).to_rgb8();
        let mut jpeg_bytes = Vec::new();
        let mut cursor = Cursor::new(&mut jpeg_bytes);
        let encoder = JpegEncoder::new_with_quality(&mut cursor, quality.clamp(30, 95));
        rgb.write_with_encoder(encoder).ok()?;

        Some(BASE64_STANDARD.encode(jpeg_bytes))
    }
}
