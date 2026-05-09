use std::fs;
use std::path::{Path, PathBuf};

use crate::{http_client, logger};

#[derive(Copy, Clone)]
pub(crate) enum ArtworkKind {
    Cover,
    Hero,
    Logo,
    Badge,
}

impl ArtworkKind {
    fn max_dims(self) -> (u32, u32) {
        match self {
            ArtworkKind::Cover => (1024, 1536),
            ArtworkKind::Hero => (1920, 1080),
            ArtworkKind::Logo => (512, 512),
            ArtworkKind::Badge => (256, 256),
        }
    }
}

pub(crate) fn artwork_kind_from_label(label: &str) -> ArtworkKind {
    match label {
        "cover" => ArtworkKind::Cover,
        "hero" => ArtworkKind::Hero,
        "logo" => ArtworkKind::Logo,
        "icon" => ArtworkKind::Logo,
        _ => ArtworkKind::Cover,
    }
}

fn decode_image_no_limits(bytes: &[u8]) -> Result<image::DynamicImage, String> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("Unable to read image header: {error}"))?;
    reader.no_limits();
    reader
        .decode()
        .map_err(|error| format!("Unable to decode image: {error}"))
}

pub(crate) fn save_bytes_as_webp(
    bytes: &[u8],
    target: &Path,
    kind: ArtworkKind,
) -> Result<PathBuf, String> {
    let webp_target = target.with_extension("webp");
    let mut img = decode_image_no_limits(bytes)?;
    let (w, h) = (img.width(), img.height());
    let (max_w, max_h) = kind.max_dims();
    if w > max_w && h > max_h {
        let scale = (max_w as f64 / w as f64).max(max_h as f64 / h as f64);
        let new_w = ((w as f64 * scale).round() as u32).max(1);
        let new_h = ((h as f64 * scale).round() as u32).max(1);
        img = img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3);
    }

    let rgba = img.to_rgba8();
    let encoder = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
    let quality: f32 = match kind {
        ArtworkKind::Cover | ArtworkKind::Hero => 80.0,
        ArtworkKind::Logo => 90.0,
        ArtworkKind::Badge => 88.0,
    };
    let webp_bytes = encoder.encode(quality);
    fs::write(&webp_target, &*webp_bytes).map_err(|error| {
        format!(
            "Unable to write {} as WebP: {error}",
            webp_target.display()
        )
    })?;
    Ok(webp_target)
}

pub(crate) fn download_to(
    url: &str,
    target: &Path,
    kind: ArtworkKind,
) -> Result<String, String> {
    let final_target = target.with_extension("webp");
    if final_target.exists() {
        return Ok(final_target.to_string_lossy().to_string());
    }
    let response = http_client()?
        .get(url)
        .send()
        .map_err(|error| format!("Download failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Download HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("Download read failed: {error}"))?;
    let saved = save_bytes_as_webp(&bytes, target, kind)?;
    Ok(saved.to_string_lossy().to_string())
}

fn convert_image_path_to_webp(path: &str, kind: ArtworkKind) -> Option<String> {
    let p = Path::new(path);
    if !p.exists() {
        return None;
    }
    let bytes = match fs::read(p) {
        Ok(value) => value,
        Err(error) => {
            logger::warn(format!("convert_image_path_to_webp read {}: {error}", p.display()));
            return None;
        }
    };
    let img = match decode_image_no_limits(&bytes) {
        Ok(value) => value,
        Err(error) => {
            logger::warn(format!("convert_image_path_to_webp decode {}: {error}", p.display()));
            return None;
        }
    };
    let (w, h) = (img.width(), img.height());
    let (max_w, max_h) = kind.max_dims();
    let already_webp = path.to_lowercase().ends_with(".webp");
    let needs_resize = w > max_w && h > max_h;
    if already_webp && !needs_resize {
        return None;
    }
    let saved = match save_bytes_as_webp(&bytes, p, kind) {
        Ok(value) => value,
        Err(error) => {
            logger::warn(format!("convert_image_path_to_webp save {}: {error}", p.display()));
            return None;
        }
    };
    if saved != p {
        let _ = fs::remove_file(p);
    }
    Some(saved.to_string_lossy().to_string())
}

pub(crate) fn convert_optional_to_webp(
    field: &mut Option<String>,
    kind: ArtworkKind,
) -> bool {
    let Some(path) = field.as_ref() else {
        return false;
    };
    if let Some(new_path) = convert_image_path_to_webp(path, kind) {
        *field = Some(new_path);
        true
    } else {
        false
    }
}
