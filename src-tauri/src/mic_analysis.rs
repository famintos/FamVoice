use crate::settings;

const MIC_MIN_SILENCE_THRESHOLD_RMS: f64 = 42.0;
const MIC_MAX_SILENCE_THRESHOLD_RMS: f64 = 12.0;
const MIC_MIN_TARGET_RMS: f64 = 1200.0;
const MIC_MAX_TARGET_RMS: f64 = 2200.0;
const MIC_TARGET_PEAK: f64 = 12000.0;
const MIC_MAX_AUTO_GAIN: f64 = 8.0;
const MIC_MIN_AUTO_GAIN_TO_APPLY: f64 = 1.2;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MicAudioLevels {
    pub(crate) rms: f64,
    pub(crate) peak: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MicLevelDetails {
    pub(crate) rms_dbfs: f64,
    pub(crate) peak_percent: f64,
}

fn interpolate(min_value: f64, max_value: f64, mic_sensitivity: u8) -> f64 {
    let ratio = f64::from(mic_sensitivity.min(settings::MAX_MIC_SENSITIVITY))
        / f64::from(settings::MAX_MIC_SENSITIVITY);
    min_value + (max_value - min_value) * ratio
}

pub(crate) fn silence_threshold(mic_sensitivity: u8) -> f64 {
    interpolate(
        MIC_MIN_SILENCE_THRESHOLD_RMS,
        MIC_MAX_SILENCE_THRESHOLD_RMS,
        mic_sensitivity,
    )
}

fn target_rms(mic_sensitivity: u8) -> f64 {
    interpolate(MIC_MIN_TARGET_RMS, MIC_MAX_TARGET_RMS, mic_sensitivity)
}

pub(crate) fn analyze(samples: &[i16]) -> MicAudioLevels {
    if samples.is_empty() {
        return MicAudioLevels {
            rms: 0.0,
            peak: 0.0,
        };
    }

    let mut sum_squares = 0.0;
    let mut peak: f64 = 0.0;

    for &sample in samples {
        let sample_f64 = sample as f64;
        sum_squares += sample_f64 * sample_f64;
        peak = peak.max((sample as i32).abs() as f64);
    }

    MicAudioLevels {
        rms: (sum_squares / samples.len() as f64).sqrt(),
        peak,
    }
}

pub(crate) fn dbfs(level: f64) -> f64 {
    if level <= 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * (level / i16::MAX as f64).log10()
    }
}

pub(crate) fn level_details(levels: MicAudioLevels) -> MicLevelDetails {
    MicLevelDetails {
        rms_dbfs: dbfs(levels.rms),
        peak_percent: (levels.peak / i16::MAX as f64 * 100.0).clamp(0.0, 100.0),
    }
}

pub(crate) fn should_reject_for_silence(levels: MicAudioLevels, mic_sensitivity: u8) -> bool {
    levels.rms < silence_threshold(mic_sensitivity)
}

fn auto_gain(levels: MicAudioLevels, mic_sensitivity: u8) -> Option<f64> {
    if levels.rms <= 0.0 || levels.peak <= 0.0 {
        return None;
    }

    let target_rms = target_rms(mic_sensitivity);
    if levels.rms >= target_rms {
        return None;
    }

    let gain = (target_rms / levels.rms)
        .min(MIC_TARGET_PEAK / levels.peak)
        .min(MIC_MAX_AUTO_GAIN);

    (gain > MIC_MIN_AUTO_GAIN_TO_APPLY).then_some(gain)
}

fn apply_gain(samples: &mut [i16], gain: f64) {
    for sample in samples {
        *sample = ((*sample as f64) * gain)
            .round()
            .clamp(i16::MIN as f64, i16::MAX as f64) as i16;
    }
}

pub(crate) fn normalize_quiet_audio(samples: &mut [i16], mic_sensitivity: u8) -> Option<f64> {
    let gain = auto_gain(analyze(samples), mic_sensitivity)?;
    apply_gain(samples, gain);
    Some(gain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mic_sensitivity_lowers_the_silence_threshold() {
        assert!(silence_threshold(100) < silence_threshold(0));
    }

    #[test]
    fn mic_rejects_near_silent_audio_at_default_sensitivity() {
        let levels = analyze(&[0, 10, -10, 0, 5, -5]);

        assert!(should_reject_for_silence(
            levels,
            settings::DEFAULT_MIC_SENSITIVITY,
        ));
    }

    #[test]
    fn mic_high_sensitivity_keeps_quiet_speech_that_low_sensitivity_rejects() {
        let levels = analyze(&[20, -20, 15, -15, 25, -25]);

        assert!(should_reject_for_silence(levels, 0));
        assert!(!should_reject_for_silence(levels, 100));
    }

    #[test]
    fn mic_normalize_quiet_audio_boosts_samples_with_a_gain_cap() {
        let mut samples = vec![100, -100, 50, -50];

        let gain = normalize_quiet_audio(&mut samples, 100).unwrap();

        assert_eq!(gain, 8.0);
        assert_eq!(samples, vec![800, -800, 400, -400]);
    }

    #[test]
    fn mic_level_details_include_dbfs_and_peak_percent() {
        let details = level_details(MicAudioLevels {
            rms: 1024.0,
            peak: 8192.0,
        });

        assert!(details.rms_dbfs < 0.0);
        assert!(details.rms_dbfs > -40.0);
        assert!((details.peak_percent - 25.0).abs() < 0.1);
    }
}
