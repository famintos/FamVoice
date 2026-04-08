use nnnoiseless::DenoiseState;

const UPSAMPLE_FACTOR: usize = 3;
const FRAME_SIZE: usize = DenoiseState::FRAME_SIZE;
const LEAD_IN_SAMPLES_16KHZ: usize = FRAME_SIZE / UPSAMPLE_FACTOR;

pub struct NoiseSuppressor {
    denoiser: Box<DenoiseState<'static>>,
}

impl Default for NoiseSuppressor {
    fn default() -> Self {
        Self::new()
    }
}

impl NoiseSuppressor {
    pub fn new() -> Self {
        Self {
            denoiser: DenoiseState::new(),
        }
    }

    pub fn process_samples(&mut self, samples: &[i16]) -> Vec<i16> {
        if samples.is_empty() {
            return Vec::new();
        }

        let upsampled = upsample_to_48khz(samples);
        if upsampled.len() < FRAME_SIZE {
            return samples.to_vec();
        }

        let mut denoised = denoise_48khz(self.denoiser.as_mut(), &upsampled);
        denoised.truncate(upsampled.len());

        if denoised.len() <= FRAME_SIZE {
            return samples.to_vec();
        }

        let denoised = &denoised[FRAME_SIZE..];
        let downsampled = downsample_to_16khz(denoised);
        if downsampled.is_empty() {
            samples.to_vec()
        } else {
            let mut output = Vec::with_capacity(samples.len());
            let lead_in_len = LEAD_IN_SAMPLES_16KHZ.min(samples.len());
            output.extend_from_slice(&samples[..lead_in_len]);
            output.extend_from_slice(&downsampled);
            if output.len() < samples.len() {
                output.extend_from_slice(&samples[output.len()..]);
            }
            output.truncate(samples.len());
            output
        }
    }
}

pub fn maybe_apply_noise_suppression(
    samples: &mut Vec<i16>,
    enabled: bool,
) -> Result<bool, String> {
    if !enabled {
        return Ok(false);
    }

    let original = samples.clone();
    let mut suppressor = NoiseSuppressor::new();
    let processed = suppressor.process_samples(&original);
    if processed == original {
        return Ok(false);
    }

    *samples = processed;
    Ok(true)
}

fn upsample_to_48khz(samples: &[i16]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    if samples.len() == 1 {
        let sample = samples[0] as f32;
        return vec![sample; UPSAMPLE_FACTOR];
    }

    let mut output = Vec::with_capacity(samples.len() * UPSAMPLE_FACTOR);

    for window in samples.windows(2) {
        let current = window[0] as f32;
        let next = window[1] as f32;
        let delta = (next - current) / UPSAMPLE_FACTOR as f32;

        output.push(current);
        output.push(current + delta);
        output.push(current + delta * 2.0);
    }

    let last = samples[samples.len() - 1] as f32;
    output.extend_from_slice(&[last; UPSAMPLE_FACTOR]);
    output
}

fn denoise_48khz(denoiser: &mut DenoiseState<'static>, samples: &[f32]) -> Vec<f32> {
    let mut output = Vec::with_capacity(samples.len() + FRAME_SIZE);
    let mut input_frame = [0.0f32; FRAME_SIZE];
    let mut output_frame = [0.0f32; FRAME_SIZE];

    for chunk in samples.chunks(FRAME_SIZE) {
        input_frame.fill(0.0);
        input_frame[..chunk.len()].copy_from_slice(chunk);
        denoiser.process_frame(&mut output_frame, &input_frame);
        output.extend_from_slice(&output_frame);
    }

    output
}

fn downsample_to_16khz(samples: &[f32]) -> Vec<i16> {
    let mut output = Vec::with_capacity(samples.len() / UPSAMPLE_FACTOR);

    for chunk in samples.chunks_exact(UPSAMPLE_FACTOR) {
        let averaged = (chunk[0] + chunk[1] + chunk[2]) / UPSAMPLE_FACTOR as f32;
        output.push(averaged.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsample_to_48khz_triples_length_and_interpolates() {
        let output = upsample_to_48khz(&[0, 3_000]);

        assert_eq!(output.len(), 6);
        assert!((output[0] - 0.0).abs() < f32::EPSILON);
        assert!((output[1] - 1_000.0).abs() < 1.0);
        assert!((output[2] - 2_000.0).abs() < 1.0);
        assert!((output[3] - 3_000.0).abs() < 1.0);
        assert!((output[4] - 3_000.0).abs() < 1.0);
        assert!((output[5] - 3_000.0).abs() < 1.0);
    }

    #[test]
    fn downsample_to_16khz_averages_triplets() {
        let output = downsample_to_16khz(&[0.0, 300.0, 600.0, -300.0, -300.0, -300.0]);

        assert_eq!(output.len(), 2);
        assert!(output[0] >= 0);
        assert!(output[1] <= 0);
    }

    #[test]
    fn maybe_apply_noise_suppression_returns_false_when_disabled() {
        let mut samples = vec![1, 2, 3];

        let changed = maybe_apply_noise_suppression(&mut samples, false).unwrap();

        assert!(!changed);
        assert_eq!(samples, vec![1, 2, 3]);
    }

    #[test]
    fn process_samples_keeps_short_inputs_unchanged() {
        let mut suppressor = NoiseSuppressor::new();
        let samples = vec![1, 2, 3, 4, 5];

        assert_eq!(suppressor.process_samples(&samples), samples);
    }
}
