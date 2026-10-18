//! The AI Visual model: where it lives, what it is, and fetching it.
//!
//! Like DisplayWidget's engine download, the model is fetched only when the
//! user asks for it (the AI Visual "Download" button / `sumi ai download`).
//! Files are checked against their published SHA-256 before use.

use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// OpenAI CLIP ViT-B/32 (MIT licence), ONNX export by Xenova on Hugging Face.
pub const MODEL_ID: &str = "openai/clip-vit-base-patch32";

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ModelFile {
    pub name: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

/// The vision encoder Sumi runs (fp32: the safest operator set for tract).
pub const VISION: ModelFile = ModelFile {
    name: "vision_model.onnx",
    url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model.onnx",
    size: 351_685_709,
    sha256: "fd6e1402a588279d1723c7534d4bcba5bc0b14b47dfab0e46f8c47b8270d7d40",
};

/// Text encoder + tokenizer: only needed to regenerate the prompt embeddings
/// (`sumi-embed-prompts`, a development tool); users never download them.
pub const TEXT: ModelFile = ModelFile {
    name: "text_model.onnx",
    url: "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/text_model.onnx",
    size: 254_058_553,
    sha256: "3f6571f5bad13a97c469c1622e1cfc4d9aef78b79fdbfcff804ca357bfada8cc",
};

pub fn model_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("sumi").join("models").join("clip-vit-base-patch32")
}

pub fn vision_path() -> PathBuf {
    model_dir().join(VISION.name)
}

pub fn is_installed() -> bool {
    std::fs::metadata(vision_path()).is_ok_and(|m| m.len() == VISION.size)
}

fn sha256_file(path: &std::path::Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

/// Download `file` into the model directory, verifying size and SHA-256.
/// `progress(done, total)` is called as bytes arrive.
pub fn download(file: &ModelFile, mut progress: impl FnMut(u64, u64)) -> Result<PathBuf> {
    let dir = model_dir();
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(file.name);
    if std::fs::metadata(&dest).is_ok_and(|m| m.len() == file.size) && sha256_file(&dest)? == file.sha256 {
        progress(file.size, file.size);
        return Ok(dest);
    }
    let part = dir.join(format!("{}.part", file.name));
    let resp = ureq::get(file.url).call().with_context(|| format!("downloading {}", file.url))?;
    let mut body = resp.into_body().into_reader();
    let mut out = std::fs::File::create(&part)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    loop {
        let n = body.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        h.update(&buf[..n]);
        done += n as u64;
        progress(done, file.size);
        if done > file.size {
            break;
        }
    }
    out.sync_all()?;
    let got = format!("{:x}", h.finalize());
    if done != file.size || got != file.sha256 {
        let _ = std::fs::remove_file(&part);
        bail!("{}: got {done} bytes with SHA-256 {got}, expected {} bytes {}", file.name, file.size, file.sha256);
    }
    std::fs::rename(&part, &dest)?;
    Ok(dest)
}

/// Remove the downloaded model.
pub fn remove() -> Result<()> {
    let dir = model_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}
