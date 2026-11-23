use base64::prelude::*;
use image::imageops::FilterType;
use image::{ImageBuffer, Rgba};
use std::io::Cursor;
use windows::Win32::Foundation::{BOOL, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    HDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

use crate::models::NormalizedRect;

#[link(name = "user32")]
extern "system" {
    fn PrintWindow(hwnd: HWND, hdc_blt: HDC, n_flags: u32) -> BOOL;
}

#[derive(Clone)]
pub struct CapturedFrame {
    pub width: i32,
    pub height: i32,
    pub rgba_data: Vec<u8>,
}

pub struct CaptureService;

impl CaptureService {
    /// Captures a background window frame silently using GDI PrintWindow
    pub fn capture_window(hwnd: HWND) -> Option<CapturedFrame> {
        unsafe {
            let mut client_rect = RECT::default();
            if GetClientRect(hwnd, &mut client_rect).is_err() {
                return None;
            }

            let width = client_rect.right - client_rect.left;
            let height = client_rect.bottom - client_rect.top;

            if width <= 0 || height <= 0 {
                return None;
            }

            let hdc_screen = GetDC(HWND(std::ptr::null_mut()));
            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbitmap = CreateCompatibleBitmap(hdc_screen, width, height);
            let old_bitmap = SelectObject(hdc_mem, hbitmap);

            // PW_RENDERFULLCONTENT = 2: renders DirectX backbuffer from DWM
            let pw_result = PrintWindow(hwnd, hdc_mem, 2);

            if !pw_result.as_bool() {
                // Fallback to BitBlt
                let hdc_win = GetDC(hwnd);
                let _ = BitBlt(hdc_mem, 0, 0, width, height, hdc_win, 0, 0, SRCCOPY);
                ReleaseDC(hwnd, hdc_win);
            }

            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // Negative height for top-down DIB
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

            // Cleanup GDI objects
            SelectObject(hdc_mem, old_bitmap);
            let _ = DeleteObject(hbitmap);
            let _ = DeleteDC(hdc_mem);
            ReleaseDC(HWND(std::ptr::null_mut()), hdc_screen);

            if lines <= 0 {
                return None;
            }

            // Convert BGRA to RGBA in-place
            let mut rgba_data = bgra_buf;
            for chunk in rgba_data.chunks_exact_mut(4) {
                let b = chunk[0];
                let r = chunk[2];
                chunk[0] = r;
                chunk[2] = b;
                chunk[3] = 255; // Ensure full opacity
            }

            Some(CapturedFrame {
                width,
                height,
                rgba_data,
            })
        }
    }

    /// Resizes the captured frame to a small thumbnail and encodes as JPEG base64 for UI streaming
    pub fn create_thumbnail_base64(frame: &CapturedFrame, target_w: u32, target_h: u32) -> Option<String> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(frame.width as u32, frame.height as u32, frame.rgba_data.clone())?;

        let resized = image::imageops::resize(&img, target_w, target_h, FilterType::Triangle);

        let mut jpeg_bytes = Vec::new();
        let mut cursor = Cursor::new(&mut jpeg_bytes);

        let _ = image::DynamicImage::ImageRgba8(resized)
            .write_to(&mut cursor, image::ImageFormat::Jpeg);

        Some(BASE64_STANDARD.encode(jpeg_bytes))
    }

    /// Crops a normalized bounding box area and encodes it as base64 JPEG
    pub fn crop_base64(frame: &CapturedFrame, rect: &NormalizedRect) -> Option<String> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(frame.width as u32, frame.height as u32, frame.rgba_data.clone())?;

        let x = (rect.min_x * frame.width as f32).max(0.0) as u32;
        let y = (rect.min_y * frame.height as f32).max(0.0) as u32;
        let max_w = (frame.width as u32).saturating_sub(x);
        let max_h = (frame.height as u32).saturating_sub(y);

        let w = ((rect.width() * frame.width as f32) as u32).min(max_w).max(1);
        let h = ((rect.height() * frame.height as f32) as u32).min(max_h).max(1);

        let cropped = image::imageops::crop_imm(&img, x, y, w, h).to_image();

        let mut bytes = Vec::new();
        let mut cursor = Cursor::new(&mut bytes);
        let _ = image::DynamicImage::ImageRgba8(cropped)
            .write_to(&mut cursor, image::ImageFormat::Jpeg);

        Some(BASE64_STANDARD.encode(bytes))
    }
}
