// Project: shoptet-scraper
// File: output.rs
// Description: Session directories, JPEG conversion and Excel parts.
// Author: Jan Alexandr Kopřiva
// License: MIT
use crate::{security::sanitize_filename, ImageData, Result, HEADERS};
use image::{codecs::jpeg::JpegEncoder, GenericImageView, ImageReader};
use std::{
    fs::{self, File, OpenOptions},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
#[derive(Clone)]
pub struct Logger {
    file: Arc<Mutex<File>>,
    verbose: bool,
}
impl Logger {
    pub fn new(path: &Path, verbose: bool) -> Result<Self> {
        Ok(Self {
            file: Arc::new(Mutex::new(
                OpenOptions::new().create(true).append(true).open(path)?,
            )),
            verbose,
        })
    }
    pub fn log(&self, level: &str, message: &str) {
        if level == "DEBUG" && !self.verbose {
            return;
        }
        let line = format!(
            "{} - {level} - {message}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S,%3f")
        );
        eprintln!("{line}");
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{line}");
        }
    }
    pub fn info(&self, message: &str) {
        self.log("INFO", message);
    }
    pub fn error(&self, message: &str) {
        self.log("ERROR", message);
    }
    pub fn warn(&self, message: &str) {
        self.log("WARNING", message);
    }
    pub fn debug(&self, message: &str) {
        self.log("DEBUG", message);
    }
}
pub fn session_path(base: &Path, today: &str) -> Result<PathBuf> {
    fs::create_dir_all(base)?;
    for number in 1.. {
        let path = base.join(format!("{today}_{number:03}"));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!()
}
pub fn jpeg(bytes: &[u8], path: &Path) -> Result<()> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let (w, h) = reader.into_dimensions()?;
    if u64::from(w) * u64::from(h) > 50_000_000 {
        return Err("Image exceeds 50000000 pixels".into());
    }
    let image = image::load_from_memory(bytes)?;
    let rgb = image::RgbImage::from_fn(w, h, |x, y| {
        let p = image.get_pixel(x, y).0;
        let alpha = u32::from(p[3]);
        image::Rgb(
            [0, 1, 2].map(|i| ((u32::from(p[i]) * alpha + 255 * (255 - alpha) + 127) / 255) as u8),
        )
    });
    JpegEncoder::new_with_quality(File::create(path)?, 85).encode_image(&rgb)?;
    Ok(())
}
pub fn image_filename(data: &ImageData) -> String {
    format!(
        "{}_{}_{}.jpg",
        sanitize_filename(&data.name),
        sanitize_filename(&data.number),
        data.suffix
    )
}
pub fn save_workbook(rows: &[Vec<String>], path: &Path, part: usize) -> Result<()> {
    let mut wb = rust_xlsxwriter::Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("Sheet")?;
    let mut widths: Vec<_> = HEADERS.iter().map(|s| s.chars().count()).collect();
    for (col, value) in HEADERS.iter().enumerate() {
        ws.write_string(0, col as u16, *value)?;
    }
    for (r, row) in rows.iter().enumerate() {
        for (col, value) in row.iter().enumerate() {
            widths[col] = widths[col].max(value.chars().count());
            ws.write_string((r + 1) as u32, col as u16, value)?;
        }
    }
    for (col, width) in widths.iter().enumerate() {
        ws.set_column_width(col as u16, (*width + 2).min(255) as f64)?;
    }
    wb.save(path)?;
    println!("Saved part {part} to {}", path.display());
    Ok(())
}
