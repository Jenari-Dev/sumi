//! Frame → CLIP input: area-averaged resize of the short side to 224, centre
//! crop, normalisation with CLIP's mean/std, NCHW float tensor.

/// Pixel layouts KWin hands out (QImage formats).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// QImage RGB32 / ARGB32(_Premultiplied): little-endian bytes B, G, R, A.
    Bgra,
    /// QImage RGBX8888 / RGBA8888(_Premultiplied): bytes R, G, B, A.
    Rgba,
}

impl PixelFormat {
    /// From a QImage::Format number, if it is a 32-bit layout we read.
    pub fn from_qimage(format: u32) -> Option<Self> {
        match format {
            4..=6 => Some(PixelFormat::Bgra),
            16..=18 => Some(PixelFormat::Rgba),
            _ => None,
        }
    }
}

/// A captured frame (borrowed pixels).
pub struct Frame<'a> {
    pub data: &'a [u8],
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub format: PixelFormat,
}

pub const SIZE: usize = 224;
const MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const STD: [f32; 3] = [0.268_629_54, 0.261_302_58, 0.275_777_1];

impl Frame<'_> {
    fn rgb(&self, x: usize, y: usize) -> [f32; 3] {
        let i = y * self.stride + x * 4;
        let p = &self.data[i..i + 4];
        match self.format {
            PixelFormat::Bgra => [p[2] as f32, p[1] as f32, p[0] as f32],
            PixelFormat::Rgba => [p[0] as f32, p[1] as f32, p[2] as f32],
        }
    }
}

/// CLIP pixel values, shape [1, 3, 224, 224], channel-major.
pub fn to_tensor(f: &Frame) -> Vec<f32> {
    assert!(f.width > 0 && f.height > 0 && f.data.len() >= f.stride * f.height);
    // Scale so the short side is 224, then take the centred 224×224 window.
    let scale = f.width.min(f.height) as f64 / SIZE as f64;
    let crop_w = SIZE as f64 * scale;
    let x0 = (f.width as f64 - crop_w) / 2.0;
    let y0 = (f.height as f64 - crop_w) / 2.0;
    let mut out = vec![0f32; 3 * SIZE * SIZE];
    for oy in 0..SIZE {
        // The epsilons keep rounding error from pulling in a pixel outside the crop.
        let sy0 = (y0 + oy as f64 * scale + 1e-6).floor() as usize;
        let sy1 = ((y0 + (oy + 1) as f64 * scale - 1e-6).ceil() as usize).clamp(sy0 + 1, f.height);
        for ox in 0..SIZE {
            let sx0 = (x0 + ox as f64 * scale + 1e-6).floor() as usize;
            let sx1 = ((x0 + (ox + 1) as f64 * scale - 1e-6).ceil() as usize).clamp(sx0 + 1, f.width);
            // Area average over the source block (step 2 on big blocks keeps it cheap).
            let step = if (sx1 - sx0) * (sy1 - sy0) > 64 { 2 } else { 1 };
            let mut acc = [0f32; 3];
            let mut n = 0f32;
            for sy in (sy0..sy1).step_by(step) {
                for sx in (sx0..sx1).step_by(step) {
                    let p = f.rgb(sx, sy);
                    acc[0] += p[0];
                    acc[1] += p[1];
                    acc[2] += p[2];
                    n += 1.0;
                }
            }
            for c in 0..3 {
                let v = acc[c] / n / 255.0;
                out[c * SIZE * SIZE + oy * SIZE + ox] = (v - MEAN[c]) / STD[c];
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_colour_normalises_per_channel() {
        let (w, h) = (640, 360);
        let data: Vec<u8> = std::iter::repeat([0u8, 128, 255, 255]).take(w * h).flatten().collect(); // BGRA: blue 0, green 128, red 255
        let t = to_tensor(&Frame { data: &data, width: w, height: h, stride: w * 4, format: PixelFormat::Bgra });
        assert_eq!(t.len(), 3 * SIZE * SIZE);
        let r = t[0];
        let g = t[SIZE * SIZE];
        let b = t[2 * SIZE * SIZE];
        assert!((r - (1.0 - MEAN[0]) / STD[0]).abs() < 1e-4);
        assert!((g - (128.0 / 255.0 - MEAN[1]) / STD[1]).abs() < 1e-4);
        assert!((b - (0.0 - MEAN[2]) / STD[2]).abs() < 1e-4);
    }

    #[test]
    fn centre_crop_ignores_the_sides() {
        // Left and right quarters red, middle green: the 16:9 centre crop sees only green.
        let (w, h) = (1600, 900);
        let mut data = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                let green = x >= 350 && x < 1250;
                data[i..i + 4].copy_from_slice(&if green { [0, 255, 0, 255] } else { [255, 0, 0, 255] });
            }
        }
        let t = to_tensor(&Frame { data: &data, width: w, height: h, stride: w * 4, format: PixelFormat::Rgba });
        let red_max = t[..SIZE * SIZE].iter().cloned().fold(f32::MIN, f32::max);
        assert!(red_max < (0.01 - MEAN[0]) / STD[0], "no red inside the crop");
    }
}
