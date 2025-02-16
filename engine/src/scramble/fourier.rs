use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb};
use ndarray::Array2;
use num_complex::Complex64;
use rustfft::{Fft, FftPlanner};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use crate::Result;
use super::types::{FourierOptions, FrequencyRange, PaddingMode};
use face_detection::{detect_face_regions, load_face_detector};
use crate::FaceDetectionOptions;
use crate::BackgroundMode;
use image::GenericImage;
pub struct FourierScrambler {
    width: usize,
    height: usize,
    fft: std::sync::Arc<dyn Fft<f64>>,
    ifft: std::sync::Arc<dyn Fft<f64>>,
    options: FourierOptions,
    rng: StdRng,
}

impl FourierScrambler {
    pub fn new(width: usize, height: usize, options: FourierOptions, seed: Option<u64>) -> Self {
        // Determine the padded size (square) based on the maximum dimension.
        let padded_size = get_optimal_fft_size(width.max(height));
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(padded_size);
        let ifft = planner.plan_fft_inverse(padded_size);
        let rng = if let Some(seed) = seed {
            StdRng::seed_from_u64(seed)
        } else {
            StdRng::from_os_rng()
        };
        Self {
            width,
            height,
            fft,
            ifft,
            options,
            rng,
        }
    }

    /// Scrambles a single image.
    pub fn scramble(&mut self, image: &DynamicImage) -> Result<DynamicImage> {
        let (width, height) = image.dimensions();
        self.width = width as usize;
        self.height = height as usize;
        let channels = self.split_channels(image)?;
        let processed_channels: Vec<Array2<f64>> = channels
            .into_iter()
            .map(|channel| self.process_channel(channel))
            .collect::<Result<Vec<_>>>()?;
        self.combine_channels(processed_channels)
    }
    pub fn scramble_with_face_detection(
        &mut self,
        image: &DynamicImage,
        face_opts: &FaceDetectionOptions,
    ) -> Result<DynamicImage> {
        let session = load_face_detector(None)?;
        let face_regions = detect_face_regions(
            image,
            &session,
            face_opts.confidence_threshold,
            Some(face_opts.expansion_factor),
        )?;
        let (width, height) = image.dimensions();
        let mut result = image.clone();
        match face_opts.background_mode {
            BackgroundMode::Include => {
                for region in face_regions {
                    let region_width = region.x2 - region.x1;
                    let region_height = region.y2 - region.y1;
                    let mut region_img = DynamicImage::new_rgb8(region_width, region_height);
                    for y in 0..region_height {
                        for x in 0..region_width {
                            let pixel = image.get_pixel(x + region.x1, y + region.y1);
                            region_img.put_pixel(x, y, pixel);
                        }
                    }
                    let processed = self.scramble(&region_img)?;
                    for y in 0..region_height {
                        for x in 0..region_width {
                            let px = processed.get_pixel(x, y);
                            result.put_pixel(x + region.x1, y + region.y1, px);
                        }
                    }
                }
                Ok(result)
            },
            BackgroundMode::Exclude => {
                let mut new_image = DynamicImage::new_rgb8(width, height);
                for region in face_regions {
                    let region_width = region.x2 - region.x1;
                    let region_height = region.y2 - region.y1;
                    let mut region_img = DynamicImage::new_rgb8(region_width, region_height);
                    for y in 0..region_height {
                        for x in 0..region_width {
                            let pixel = image.get_pixel(x + region.x1, y + region.y1);
                            region_img.put_pixel(x, y, pixel);
                        }
                    }
                    let processed = self.scramble(&region_img)?;
                    for y in 0..region_height {
                        for x in 0..region_width {
                            let px = processed.get_pixel(x, y);
                            new_image.put_pixel(x + region.x1, y + region.y1, px);
                        }
                    }
                }
                Ok(new_image)
            }
        }
    }
    /// Processes a single channel: pads the image, computes its 2D FFT,
    /// replaces its phase while preserving the magnitude, and then computes the inverse FFT.
    fn process_channel(&mut self, channel: Array2<f64>) -> Result<Array2<f64>> {
        let padded = self.apply_padding(&channel)?;
        let n = padded.dim().0; // padded is square of size n x n
        let mut complex_data = self.to_complex(&padded);
        self.fft2d(&mut complex_data, n);
        if self.options.phase_scramble {
            self.phase_scramble(&mut complex_data);
        }
        self.ifft2d(&mut complex_data, n);
        let mut result = self.remove_padding(&complex_data, channel.dim())?;
        // Clamp the output values to [0, 1]
        for val in result.iter_mut() {
            *val = val.max(0.0).min(1.0);
        }
        Ok(result)
    }

    /// Computes the 2D FFT by applying the 1D FFT along rows then columns.
    fn fft2d(&self, data: &mut [Complex64], n: usize) {
        // FFT each row.
        for row in 0..n {
            let start = row * n;
            let end = start + n;
            self.fft.process(&mut data[start..end]);
        }
        // FFT each column.
        let mut column = vec![Complex64::new(0.0, 0.0); n];
        for col in 0..n {
            for row in 0..n {
                column[row] = data[row * n + col];
            }
            self.fft.process(&mut column);
            for row in 0..n {
                data[row * n + col] = column[row];
            }
        }
    }

    /// Computes the 2D inverse FFT by applying the 1D IFFT along rows then columns.
    /// The result is scaled by 1/(n*n).
    fn ifft2d(&self, data: &mut [Complex64], n: usize) {
        // IFFT each row.
        for row in 0..n {
            let start = row * n;
            let end = start + n;
            self.ifft.process(&mut data[start..end]);
        }
        // IFFT each column.
        let mut column = vec![Complex64::new(0.0, 0.0); n];
        for col in 0..n {
            for row in 0..n {
                column[row] = data[row * n + col];
            }
            self.ifft.process(&mut column);
            for row in 0..n {
                data[row * n + col] = column[row];
            }
        }
        // Scale the output.
        let scale = 1.0 / (n * n) as f64;
        for val in data.iter_mut() {
            *val = *val * scale;
        }
    }

    /// Replaces the phase of each frequency coefficient while preserving its magnitude.
    /// For each coefficient, a random phase is generated and the new phase is computed as:
    ///    new_phase = orig_phase + intensity * (random_phase - orig_phase)
    /// The symmetric counterpart is set to the conjugate to maintain a real inverse FFT.
    fn phase_scramble(&mut self, data: &mut [Complex64]) {
        let n = (data.len() as f64).sqrt() as usize;
        for y in 0..n {
            for x in 0..n {
                // Determine symmetric coordinates.
                let sym_y = if y == 0 { 0 } else { n - y };
                let sym_x = if x == 0 { 0 } else { n - x };
                // Process each pair only once.
                if y > sym_y || (y == sym_y && x > sym_x) {
                    continue;
                }
                let idx = y * n + x;
                let orig = data[idx];
                let mag = orig.norm();
                let orig_phase = orig.arg();
                let random_phase = self.rng.gen_range(0.0..(2.0 * std::f64::consts::PI));
                let dphase = angle_difference(random_phase, orig_phase);
                let new_phase = orig_phase + self.options.intensity as f64 * dphase;
                let new_val = Complex64::from_polar(mag, new_phase);
                data[idx] = new_val;
                if !(y == sym_y && x == sym_x) {
                    let sym_idx = sym_y * n + sym_x;
                    data[sym_idx] = new_val.conj();
                }
            }
        }
    }

    /// Converts a 2D real array to a flat vector of Complex64.
    fn to_complex(&self, real: &Array2<f64>) -> Vec<Complex64> {
        real.iter().map(|&val| Complex64::new(val, 0.0)).collect()
    }

    /// Splits the input image into three channels (normalized to [0, 1]) as 2D arrays.
    fn split_channels(&self, image: &DynamicImage) -> Result<Vec<Array2<f64>>> {
        let rgb = image.to_rgb8();
        let (width, height) = (self.width, self.height);
        let mut channels = Vec::with_capacity(3);
        for c in 0..3 {
            let mut channel = Array2::zeros((height, width));
            for y in 0..height {
                for x in 0..width {
                    let pixel = rgb.get_pixel(x as u32, y as u32);
                    channel[[y, x]] = pixel[c] as f64 / 255.0;
                }
            }
            channels.push(channel);
        }
        Ok(channels)
    }
    /// Combines three 2D arrays (for R, G, B channels) into a single image.
    fn combine_channels(&self, channels: Vec<Array2<f64>>) -> Result<DynamicImage> {
        let (width, height) = (self.width as u32, self.height as u32);
        let mut image = ImageBuffer::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let r = (channels[0][[y as usize, x as usize]] * 255.0) as u8;
                let g = (channels[1][[y as usize, x as usize]] * 255.0) as u8;
                let b = (channels[2][[y as usize, x as usize]] * 255.0) as u8;
                image.put_pixel(x, y, Rgb([r, g, b]));
            }
        }
        Ok(DynamicImage::ImageRgb8(image))
    }
    /// Pads the given channel to a square of size (padded_size x padded_size) using the chosen mode.
    fn apply_padding(&self, channel: &Array2<f64>) -> Result<Array2<f64>> {
        let (height, width) = channel.dim();
        let padded_size = get_optimal_fft_size(width.max(height));
        let mut padded = Array2::zeros((padded_size, padded_size));
        match self.options.padding_mode {
            PaddingMode::Zero => {
                for y in 0..height {
                    for x in 0..width {
                        padded[[y, x]] = channel[[y, x]];
                    }
                }
            },
            PaddingMode::Reflect => {
                // Copy original data.
                for y in 0..height {
                    for x in 0..width {
                        padded[[y, x]] = channel[[y, x]];
                    }
                }
                // Reflect horizontally.
                for y in 0..height {
                    for x in width..padded_size {
                        padded[[y, x]] = channel[[y, 2 * width - x - 1]];
                    }
                }
                // Reflect vertically.
                for y in height..padded_size {
                    for x in 0..padded_size {
                        padded[[y, x]] = padded[[2 * height - y - 1, x]];
                    }
                }
            },
            PaddingMode::Wrap => {
                for y in 0..padded_size {
                    for x in 0..padded_size {
                        padded[[y, x]] = channel[[y % height, x % width]];
                    }
                }
            },
        }
        Ok(padded)
    }
    /// Removes the padding from the inverse-transformed data.
    fn remove_padding(&self, complex_data: &[Complex64], original_dim: (usize, usize)) -> Result<Array2<f64>> {
        let (height, width) = original_dim;
        let padded_size = get_optimal_fft_size(width.max(height));
        let mut result = Array2::zeros((height, width));
        for y in 0..height {
            for x in 0..width {
                let idx = y * padded_size + x;
                result[[y, x]] = complex_data[idx].re;
            }
        }
        Ok(result)
    }
}

/// Returns the next power of two greater than or equal to `size`.
fn get_optimal_fft_size(size: usize) -> usize {
    let mut optimal_size = size;
    while !is_power_of_two(optimal_size) {
        optimal_size += 1;
    }
    optimal_size
}

fn is_power_of_two(n: usize) -> bool {
    n != 0 && (n & (n - 1)) == 0
}

/// Computes the minimal angular difference between two angles (in radians),
/// accounting for wrapping at ±π.
fn angle_difference(a: f64, b: f64) -> f64 {
    let mut diff = a - b;
    while diff > std::f64::consts::PI {
        diff -= 2.0 * std::f64::consts::PI;
    }
    while diff < -std::f64::consts::PI {
        diff += 2.0 * std::f64::consts::PI;
    }
    diff
}
