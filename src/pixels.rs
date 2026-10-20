//! PhotoCraft-compatible pixel processing; attribution: assets/licenses/photocraft-MIT.txt.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Filters {
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub exposure: f64,
    pub hue: f64,
    pub colorize: bool,
    pub colorize_saturation: f64,
}
impl Default for Filters {
    fn default() -> Self {
        Self {
            brightness: 0.,
            contrast: 0.,
            saturation: 0.,
            exposure: 0.,
            hue: 0.,
            colorize: false,
            colorize_saturation: 25.,
        }
    }
}
impl Filters {
    pub fn normalized(self) -> Self {
        Self {
            brightness: self.brightness.clamp(-150., 150.),
            contrast: self.contrast.clamp(-50., 100.),
            saturation: self.saturation.clamp(-100., 100.),
            exposure: self.exposure.clamp(-3., 3.),
            hue: self.hue.clamp(-180., 180.),
            colorize: self.colorize,
            colorize_saturation: self.colorize_saturation.clamp(0., 100.),
        }
    }
}
pub fn brightness(v: f64, b: f64) -> f64 {
    if b == 0. {
        return v;
    }
    let slope = 1.375_f64.powf(b / 50.);
    let p = if b >= 0. {
        (4.5 - 0.013 * b).max(2.)
    } else {
        5. - 0.072 * b
    };
    let v = v.clamp(0., 1.);
    (slope * v + (1. - slope) * v.powf(p)).clamp(0., 1.)
}
pub fn contrast(v: f64, c: f64) -> f64 {
    if c == 0. {
        return v;
    }
    let k = c / 128.;
    let half = |t: f64| {
        (1. - k) * 0.5 * (t.powi(3) - 2. * t * t + t)
            + (-2. * t.powi(3) + 3. * t * t) * 0.5
            + (1. + k) * 0.5 * (t.powi(3) - t * t)
    };
    let v = v.clamp(0., 1.);
    if v <= 0.5 {
        half(v / 0.5)
    } else {
        1. - half((1. - v) / 0.5)
    }
}
pub fn process(bytes: &mut [u8], raw: Filters) -> Result<(), String> {
    if bytes.len() % 4 != 0 {
        return Err("RGBA 缓冲区长度无效".into());
    }
    let f = raw.normalized();
    let gain = 2_f64.powf(f.exposure);
    let lut: [f32; 256] = std::array::from_fn(|i| {
        let tone = contrast(brightness(i as f64 / 255., f.brightness), f.contrast).clamp(0., 1.);
        (tone.powf(2.2) * gain).powf(1. / 2.2).clamp(0., 1.) as f32
    });
    let change = f.hue != 0. || f.saturation != 0. || f.colorize;
    for pixel in bytes.chunks_exact_mut(4) {
        let (mut r, mut g, mut b) = (
            lut[pixel[0] as usize] as f64,
            lut[pixel[1] as usize] as f64,
            lut[pixel[2] as usize] as f64,
        );
        if change {
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let d = max - min;
            let l = (max + min) / 2.;
            let mut h = if d < 1e-8 {
                0.
            } else if max == r {
                ((g - b) / d + if g < b { 6. } else { 0. }) / 6.
            } else if max == g {
                ((b - r) / d + 2.) / 6.
            } else {
                ((r - g) / d + 4.) / 6.
            };
            let mut s = if d < 1e-8 {
                0.
            } else {
                d / (1. - (2. * l - 1.).abs())
            };
            if f.colorize {
                h = f.hue.rem_euclid(360.) / 360.;
                s = f.colorize_saturation / 100.;
            } else {
                h = (h + f.hue / 360.).rem_euclid(1.);
                s = (s * (1. + f.saturation / 100.)).clamp(0., 1.);
            }
            let c = (1. - (2. * l - 1.).abs()) * s;
            let x = c * (1. - ((h * 6.) % 2. - 1.).abs());
            let m = l - c / 2.;
            (r, g, b) = if h < 1. / 6. {
                (c, x, 0.)
            } else if h < 2. / 6. {
                (x, c, 0.)
            } else if h < 3. / 6. {
                (0., c, x)
            } else if h < 4. / 6. {
                (0., x, c)
            } else if h < 5. / 6. {
                (x, 0., c)
            } else {
                (c, 0., x)
            };
            r += m;
            g += m;
            b += m;
        }
        pixel[0] = (r.clamp(0., 1.) * 255.).round() as u8;
        pixel[1] = (g.clamp(0., 1.) * 255.).round() as u8;
        pixel[2] = (b.clamp(0., 1.) * 255.).round() as u8;
    }
    Ok(())
}
