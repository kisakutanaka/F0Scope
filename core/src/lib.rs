// fft.rs / pitch.rs / pitch_detect.rs は FFTexample からコピーしたもの。
// pitch_detect.rs の dtft_magnitude だけ WASM での速度のために変更している(CLAUDE.md 参照)。
// デスクトップ版でだけ使う関数があるので dead_code 警告は抑える。
#[allow(dead_code)]
mod fft;
#[allow(dead_code)]
mod pitch;
#[allow(dead_code)]
mod pitch_detect;

use serde::Serialize;
use wasm_bindgen::prelude::*;

/// ホールド・ラップ表示で桁がずれないよう、読みの文字列を固定幅にそろえる(gui.rs と同じ)。
const READING_WIDTH: usize = 26;

fn format_reading(reading: Option<(f32, pitch::Pitch)>) -> String {
    let text = match reading {
        Some((frequency, pitch)) => format!("{} ({:>7.1} Hz)", pitch, frequency),
        None => "---".to_string(),
    };
    format!("{text:<READING_WIDTH$}")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Reading {
    frequency: f32,
    strength: f32,
    pitch_text: String,
    reading_text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Analysis {
    fundamental: Option<Reading>,
    strongest: Option<Reading>,
    coarse: Vec<(f32, f32)>,
    zoom: Vec<(f32, f32)>,
    fit_curve: Vec<(f32, f32)>,
}

/// gui.rs のオーディオコールバック内の処理(analyze_with_spectrum → frequency_to_pitch)と同じ。
fn analyze_samples(samples: &[f32], sample_rate: f32, reference_a4_hz: f32) -> Analysis {
    let (estimate, spectrum) = pitch_detect::analyze_with_spectrum(samples, sample_rate);
    let to_reading = |frequency: Option<f32>, strength: Option<f32>| {
        frequency.zip(strength).map(|(frequency, strength)| {
            let pitch = pitch::frequency_to_pitch(frequency, reference_a4_hz);
            Reading {
                frequency,
                strength,
                pitch_text: pitch.to_string(),
                reading_text: format_reading(Some((frequency, pitch))),
            }
        })
    };
    Analysis {
        fundamental: to_reading(estimate.fundamental_hz, estimate.fundamental_strength),
        strongest: to_reading(estimate.strongest_hz, estimate.strongest_strength),
        coarse: spectrum.coarse,
        zoom: spectrum.zoom,
        fit_curve: spectrum.fit_curve,
    }
}

#[wasm_bindgen]
pub fn analyze(samples: &[f32], sample_rate: f32, reference_a4_hz: f32) -> Result<JsValue, JsValue> {
    Ok(serde_wasm_bindgen::to_value(&analyze_samples(samples, sample_rate, reference_a4_hz))?)
}

/// 読みが無いときの表示(ラップ記録用)。
#[wasm_bindgen]
pub fn empty_reading_text() -> String {
    format_reading(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ネイティブ(cargo test)と WASM(wasm-pack test --node)の両方で同じテストを走らせる。
    macro_rules! both {
        ($(fn $name:ident() $body:block)*) => {$(
            #[cfg_attr(not(target_arch = "wasm32"), test)]
            #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
            fn $name() $body
        )*};
    }

    fn cents_between(a: f32, b: f32) -> f32 {
        1200.0 * (a / b).log2()
    }

    /// WEB_PORT_PLAN.md 7.2 のデスクトップ版実測値: (サンプルレート, 入力 Hz, 推定基音 Hz, 音名+オクターブ)
    const SINE_REFERENCES: [(f32, f32, f32, &str); 16] = [
        (44100.0, 82.4, 82.9313, "E  2"),
        (44100.0, 110.0, 110.2179, "A  2"),
        (44100.0, 220.0, 220.0177, "A  3"),
        (44100.0, 440.0, 439.9985, "A  4"),
        (44100.0, 880.0, 879.9984, "A  5"),
        (44100.0, 1046.5, 1046.5000, "C  6"),
        (44100.0, 2093.0, 2093.0002, "C  7"),
        (44100.0, 4978.0, 4978.0010, "D# 8"),
        (48000.0, 82.4, 82.4104, "E  2"),
        (48000.0, 110.0, 110.1514, "A  2"),
        (48000.0, 220.0, 220.0055, "A  3"),
        (48000.0, 440.0, 440.0045, "A  4"),
        (48000.0, 880.0, 880.0021, "A  5"),
        (48000.0, 1046.5, 1046.4977, "C  6"),
        (48000.0, 2093.0, 2093.0002, "C  7"),
        (48000.0, 4978.0, 4978.0000, "D# 8"),
    ];

    both! {
        fn format_reading_width_is_constant_regardless_of_digits() {
            let short_digits = (82.4, pitch::Pitch { note_name: "E", octave: 2, cents: 0.0 });
            let long_digits = (1046.5, pitch::Pitch { note_name: "C#", octave: 10, cents: -49.0 });

            assert_eq!(format_reading(Some(short_digits)).chars().count(), READING_WIDTH);
            assert_eq!(format_reading(Some(long_digits)).chars().count(), READING_WIDTH);
            assert_eq!(format_reading(None).chars().count(), READING_WIDTH);
        }

        fn sine_waves_match_desktop_results() {
            for (sample_rate, input_hz, expected_hz, expected_note) in SINE_REFERENCES {
                let samples = fft::generate_sine_wave(input_hz, sample_rate, 1024);
                let fundamental = analyze_samples(&samples, sample_rate, 440.0).fundamental.unwrap();

                let diff = cents_between(fundamental.frequency, expected_hz);
                assert!(diff.abs() <= 0.05, "{sample_rate}Hz/{input_hz}Hz: {} Hz ({diff} cent)", fundamental.frequency);
                assert_eq!(&fundamental.pitch_text[..4], expected_note, "{sample_rate}Hz/{input_hz}Hz");
            }
        }

        fn harmonic_signal_matches_desktop_results() {
            let sample_rate = 48000.0;
            let samples: Vec<f32> = (0..1024)
                .map(|i| {
                    let t = i as f32 / sample_rate;
                    let tone = |hz: f32| (2.0 * std::f32::consts::PI * hz * t).sin();
                    0.3 * tone(220.0) + tone(440.0) + 0.2 * tone(660.0)
                })
                .collect();
            let analysis = analyze_samples(&samples, sample_rate, 440.0);
            let fundamental = analysis.fundamental.unwrap();
            let strongest = analysis.strongest.unwrap();

            assert!(cents_between(fundamental.frequency, 220.53801).abs() <= 0.05);
            assert!(cents_between(strongest.frequency, 439.9929).abs() <= 0.05);
            assert!((fundamental.strength - 72.759995).abs() < 0.01);
            assert!((strongest.strength - 232.18135).abs() < 0.01);
        }

        fn spectrum_point_counts_match_desktop_results() {
            let samples = fft::generate_sine_wave(440.0, 48000.0, 1024);
            let analysis = analyze_samples(&samples, 48000.0, 440.0);

            assert_eq!(analysis.coarse.len(), 107);
            assert_eq!(analysis.zoom.len(), 21);
            assert_eq!(analysis.fit_curve.len(), 21);
        }

        fn reference_pitch_changes_pitch_text() {
            let cases = [
                (440.0, 440.0, "A  4   +0cent"),
                (440.0, 450.0, "A  4  +39cent"),
                (440.0, 466.16, "A# 4   -0cent"),
                (440.0, 82.4, "E  2   -0cent"),
                (440.0, 27.5, "A  0   +0cent"),
                (440.0, 4186.0, "C  8   -0cent"),
                (442.0, 440.0, "A  4   -8cent"),
                (442.0, 450.0, "A  4  +31cent"),
            ];
            for (reference_a4_hz, frequency, expected) in cases {
                assert_eq!(pitch::frequency_to_pitch(frequency, reference_a4_hz).to_string(), expected);
            }
        }
    }
}
