// Copyright (c) 2025 Cua AI, Inc.
// SPDX-License-Identifier: MIT
// Adapted from platform-macos/src/capture.rs and cursor-overlay capture_exclusion.rs; see NOTICE.
use super::super::{native, policy::Display};
use super::windows::{self, Window};
use base64::{engine::general_purpose::STANDARD, Engine};
use screencapturekit::{
    cm::{CMSampleBufferExt, SCFrameStatus},
    prelude::*,
};

pub fn capture(pid: i32) -> Result<(native::Frame, Window), String> {
    if native::permissions()["screen"] != true {
        return Err("Grant Screen Recording in System Settings".into());
    }
    let content = SCShareableContent::create()
        .with_on_screen_windows_only(false)
        .get()
        .map_err(|e| format!("SCK window list: pid={pid}, {e}"))?;
    let window = windows::select(&content, pid)?;
    let target = windows::describe(&window);
    // Desktop-independent window filter: occluders, HUD and all overlay windows are excluded by construction.
    let filter = SCContentFilter::create()
        .with_window(&window)
        .build()
        .map_err(|e| e.to_string())?;
    let scale = content
        .displays()
        .iter()
        .find(|d| {
            let r = d.frame();
            target.origin[0] >= r.origin.x
                && target.origin[0] < r.origin.x + r.size.width
                && target.origin[1] >= r.origin.y
                && target.origin[1] < r.origin.y + r.size.height
        })
        .map(|d| {
            core_graphics::display::CGDisplay::new(d.display_id()).pixels_wide() as f64
                / d.frame().size.width
        })
        .unwrap_or(1.);
    if !scale.is_finite() || scale <= 0. || target.size.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return Err(format!(
            "Capture geometry: window={}, size={:?}, scale={scale}",
            target.id, target.size
        ));
    }
    let width = (target.size[0] * scale).round() as u32;
    let height = (target.size[1] * scale).round() as u32;
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err(format!("Capture dimensions exceed limit: window={}, width={width}, height={height}, limit=16384", target.id));
    }
    let config = SCStreamConfiguration::new()
        .with_width(width)
        .with_height(height)
        .with_shows_cursor(false);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let mut stream = SCStream::new(&filter, &config).map_err(|e| e.to_string())?;
    stream
        .add_output_handler(
            move |sample: CMSampleBuffer, kind: SCStreamOutputType| {
                if kind == SCStreamOutputType::Screen
                    && sample.frame_status() == Some(SCFrameStatus::Complete)
                {
                    if let Ok(image) = sample.cg_image() {
                        let result = unsafe {
                            native::encode_png(image.as_ptr().cast()).and_then(|png| {
                                native::thumbnail(image.as_ptr().cast(), width, height)
                                    .map(|thumb| (png, thumb))
                            })
                        };
                        let _ = tx.try_send(result);
                    }
                }
            },
            SCStreamOutputType::Screen,
        )
        .map_err(|e| e.to_string())?;
    stream.start_capture().map_err(|e| e.to_string())?;
    let result = rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| {
            format!(
                "Window capture: pid={pid}, window={}, on-screen={}, wait=5s, {e}",
                target.id, target.on_screen
            )
        });
    let stopped = stream.stop_capture().map_err(|e| e.to_string());
    let (png, thumbnail) = result??;
    stopped?;
    Ok((
        native::Frame {
            png: STANDARD.encode(png),
            thumbnail: STANDARD.encode(thumbnail),
            display: Display {
                origin: target.origin,
                width: target.size[0],
                height: target.size[1],
                pixel_width: width,
                pixel_height: height,
            },
        },
        target,
    ))
}
