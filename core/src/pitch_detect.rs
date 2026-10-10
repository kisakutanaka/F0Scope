use crate::fft;
use std::f32::consts::PI;

const COARSE_CANDIDATES: usize = 5;
/// 候補と認めるための、最強候補に対する振幅の相対しきい値。
/// これを下回る候補はノイズとみなして基本周波数の候補から除外する。
const FUNDAMENTAL_THRESHOLD_RATIO: f32 = 0.2;
/// 局所FFTで見る範囲: 粗いFFTのビン幅の何倍を左右に探索するか。
const ZOOM_SEARCH_WIDTH_IN_BINS: f32 = 1.0;
const ZOOM_SEARCH_POINTS: usize = 21;
/// 可視化用に粗いFFTスペクトルを送る上限周波数。音楽的に意味のある基音・倍音は
/// だいたいこの範囲に収まるため、ナイキスト周波数まで全部送って画面を間延びさせない。
const COARSE_SPECTRUM_DISPLAY_MAX_HZ: f32 = 5000.0;

/// ピッチ推定の結果。基音(F0)と、単純に最も強かったピークの両方を残す。
/// 倍音を拾ってしまっている場合、両者が異なる(だいたい整数倍の)値になるため、
/// 見比べることで検出がうまくいっているかを判断しやすくなる。
/// `_strength`は、GUI側でピークホールド(直近数秒で最も強かった読みを保持する機能)の
/// 判定に使うための、粗いFFT時点での振幅(呼び出しをまたいだ相対比較にのみ使う値)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitchEstimate {
    /// 倍音誤検出を避けるための選定ロジックを通した、基音(F0)の高精度な推定値。
    pub fundamental_hz: Option<f32>,
    pub fundamental_strength: Option<f32>,
    /// 選定ロジックを通さない、単純に最も強かったピークの高精度な推定値。
    pub strongest_hz: Option<f32>,
    pub strongest_strength: Option<f32>,
}

/// 可視化用に残しておく、基音候補まわりの解析途中のスナップショット。
/// 「粗FFT(青)→局所FFT(赤)→フィッティング放物線(黄)」をそのままグラフに描ける形。
#[derive(Debug, Clone, Default)]
pub struct SpectrumSnapshot {
    /// 粗いFFTの(周波数, 振幅)。`COARSE_SPECTRUM_DISPLAY_MAX_HZ`以下のみ。
    pub coarse: Vec<(f32, f32)>,
    /// 基音候補まわりの局所FFT(ズームサーチ)の(周波数, 振幅)。候補がなければ空。
    pub zoom: Vec<(f32, f32)>,
    /// 局所FFTのピーク付近に当てはめた放物線を、細かくサンプリングした点列。
    pub fit_curve: Vec<(f32, f32)>,
}

/// 粗いFFT→候補検出→局所FFT→放物線補間フィッティングの4段階で、
/// サンプル列から基音と最強ピークの周波数を高精度に推定する。
/// 候補が見つからない(無音に近いなど)場合はそれぞれ`None`になる。
pub fn estimate_pitch(samples: &[f32], sample_rate: f32) -> PitchEstimate {
    analyze(samples, sample_rate).0
}

/// `estimate_pitch`と同じ推定に加えて、GUIでの可視化に使う中間データ
/// (粗FFT・局所FFT・フィッティング曲線)も一緒に返す。
pub fn analyze_with_spectrum(samples: &[f32], sample_rate: f32) -> (PitchEstimate, SpectrumSnapshot) {
    analyze(samples, sample_rate)
}

fn analyze(samples: &[f32], sample_rate: f32) -> (PitchEstimate, SpectrumSnapshot) {
    // 粗FFT・局所FFTの両方に同じ窓をかけた信号を使う。窓をかけないと、解析窓の端での
    // 不連続によるスペクトル漏れが両方の段階に影響し、特に低い周波数で誤差が大きくなる。
    let windowed = fft::hann_window(samples);

    let magnitudes = fft::magnitude_spectrum(&windowed);
    let candidates = fft::find_peak_candidates(&magnitudes, sample_rate, COARSE_CANDIDATES);

    let bin_width = sample_rate / windowed.len() as f32;
    let half_width = bin_width * ZOOM_SEARCH_WIDTH_IN_BINS;
    // 局所FFT+放物線補間で精密化した周波数だけを必要とする呼び出し(最強ピーク側)向け。
    let refine = |coarse_freq: f32| -> f32 {
        let zoom_points = zoom_search(&windowed, sample_rate, coarse_freq, half_width, ZOOM_SEARCH_POINTS);
        parabolic_peak_refine(&zoom_points).0
    };

    let fundamental = select_fundamental(&candidates);
    let strongest = strongest_candidate(&candidates);

    // 基音候補についてのみ、可視化用に局所FFTの点列とフィッティング曲線を保持しておく。
    let (fundamental_hz, zoom, fit_curve) = match fundamental {
        Some((coarse_freq, _)) => {
            let zoom_points =
                zoom_search(&windowed, sample_rate, coarse_freq, half_width, ZOOM_SEARCH_POINTS);
            let (refined_freq, fit_curve) = parabolic_peak_refine(&zoom_points);
            (Some(refined_freq), zoom_points, fit_curve)
        }
        None => (None, Vec::new(), Vec::new()),
    };

    let bin_hz = sample_rate / magnitudes.len() as f32;
    let coarse = magnitudes
        .iter()
        .enumerate()
        .map(|(i, &magnitude)| (i as f32 * bin_hz, magnitude))
        .take_while(|&(freq, _)| freq <= COARSE_SPECTRUM_DISPLAY_MAX_HZ)
        .collect();

    let estimate = PitchEstimate {
        fundamental_hz,
        fundamental_strength: fundamental.map(|(_, magnitude)| magnitude),
        strongest_hz: strongest.map(|(freq, _)| refine(freq)),
        strongest_strength: strongest.map(|(_, magnitude)| magnitude),
    };

    (
        estimate,
        SpectrumSnapshot {
            coarse,
            zoom,
            fit_curve,
        },
    )
}

/// 候補の中から単純に最も振幅が強いものを(周波数, 振幅)で返す(倍音誤検出の回避は行わない)。
fn strongest_candidate(candidates: &[(f32, f32)]) -> Option<(f32, f32)> {
    candidates
        .iter()
        .copied()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
}

/// 候補(周波数昇順)の中から基本周波数らしいものを(周波数, 振幅)で選ぶ。
/// 最強候補の振幅の`FUNDAMENTAL_THRESHOLD_RATIO`以上を持つ候補のうち、
/// 最も周波数が低いものを基本周波数とみなす(強い倍音を誤って基本周波数と
/// 判断しないようにするため)。
fn select_fundamental(candidates: &[(f32, f32)]) -> Option<(f32, f32)> {
    let max_magnitude = candidates
        .iter()
        .map(|&(_, magnitude)| magnitude)
        .fold(0.0f32, f32::max);
    if max_magnitude <= 0.0 {
        return None;
    }

    let threshold = max_magnitude * FUNDAMENTAL_THRESHOLD_RATIO;
    candidates
        .iter()
        .copied()
        .find(|&(_, magnitude)| magnitude >= threshold)
}

/// 指定した周波数における信号のDTFT振幅を、元のサンプル列に対して直接計算する。
/// FFTのようにビン(サンプル数で決まる離散周波数)に縛られず、任意の周波数の
/// スペクトル強度を求められるため、追加のサンプルなしに狭い範囲を細かく調べる
/// 「ズームFFT」として使える(Goertzelアルゴリズムと等価な計算)。
/// 位相 ωn の cos/sin はサンプルごとに計算せず、1サンプル分の回転を掛けて進める
/// (WASM では cos/sin がソフトウェア実装で遅いため)。誤差が蓄積しないよう f64 で計算する。
fn dtft_magnitude(samples: &[f32], sample_rate: f32, frequency: f32) -> f32 {
    let omega = 2.0 * PI * frequency / sample_rate;
    let (step_sin, step_cos) = (omega as f64).sin_cos();
    let (mut cos, mut sin) = (1.0f64, 0.0f64);
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for &x in samples {
        re += x as f64 * cos;
        im -= x as f64 * sin;
        (cos, sin) = (cos * step_cos - sin * step_sin, sin * step_cos + cos * step_sin);
    }
    (re * re + im * im).sqrt() as f32
}

/// `center_freq`を中心に`±half_width_hz`の範囲を`num_points`等分し、
/// 各点でのDTFT振幅を(周波数, 振幅)として返す。
fn zoom_search(
    samples: &[f32],
    sample_rate: f32,
    center_freq: f32,
    half_width_hz: f32,
    num_points: usize,
) -> Vec<(f32, f32)> {
    let start = center_freq - half_width_hz;
    let step = (half_width_hz * 2.0) / (num_points - 1) as f32;

    (0..num_points)
        .map(|i| {
            let freq = start + step * i as f32;
            (freq, dtft_magnitude(samples, sample_rate, freq))
        })
        .collect()
}

/// フィッティング曲線を可視化用にサンプリングする点の数。
const FIT_CURVE_SAMPLES: usize = 20;

/// (周波数, 振幅)の等間隔な点列から最大点を探し、その両隣の点を使って
/// 放物線補間することで、点の間隔より細かい精度でピーク周波数を求める。
/// 合わせて、当てはめた放物線を可視化用に細かくサンプリングした曲線も返す。
/// ピークが探索範囲の端にある場合は補間できないため、その点だけの曲線を返す。
fn parabolic_peak_refine(points: &[(f32, f32)]) -> (f32, Vec<(f32, f32)>) {
    let (peak_index, &(peak_freq, peak_magnitude)) = points
        .iter()
        .enumerate()
        .max_by(|a, b| a.1 .1.partial_cmp(&b.1 .1).unwrap())
        .expect("空の探索範囲です");

    if peak_index == 0 || peak_index == points.len() - 1 {
        return (peak_freq, vec![(peak_freq, peak_magnitude)]);
    }

    let (f_left, m_left) = points[peak_index - 1];
    let (f_center, m_center) = points[peak_index];
    let (f_right, m_right) = points[peak_index + 1];

    let denom = m_left - 2.0 * m_center + m_right;
    if denom.abs() < f32::EPSILON {
        return (f_center, vec![(f_left, m_left), (f_center, m_center), (f_right, m_right)]);
    }

    let step = f_center - f_left;
    let offset = 0.5 * (m_left - m_right) / denom;
    let refined_freq = f_center + offset * step;

    // 3点(u=-1,0,+1、u=(x-f_center)/step)を通る放物線 y = a*u^2 + b*u + c。
    let a = 0.5 * denom;
    let b = 0.5 * (m_right - m_left);
    let c = m_center;

    let curve = (0..=FIT_CURVE_SAMPLES)
        .map(|i| {
            let u = -1.0 + 2.0 * i as f32 / FIT_CURVE_SAMPLES as f32;
            let freq = f_center + u * step;
            let magnitude = a * u * u + b * u + c;
            (freq, magnitude)
        })
        .collect();

    (refined_freq, curve)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refines_pure_tone_far_below_bin_width() {
        let sample_rate = 44100.0;
        let num_samples = 4096;
        let frequency = 440.0;

        let samples = fft::generate_sine_wave(frequency, sample_rate, num_samples);
        let estimated = estimate_pitch(&samples, sample_rate)
            .fundamental_hz
            .expect("ピークが検出されるはず");

        let bin_width = sample_rate / num_samples as f32;
        let error = (estimated - frequency).abs();
        assert!(
            error < bin_width * 0.1,
            "error {error}Hz should be much smaller than bin width {bin_width}Hz"
        );
    }

    #[test]
    fn picks_fundamental_over_a_stronger_harmonic() {
        let sample_rate = 44100.0;
        let num_samples = 4096;
        let fundamental = 220.0;

        // 基本波(220Hz)より2倍音(440Hz)の方を強くし、倍音の誤検出を誘発する意地悪な信号。
        let samples: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate;
                0.3 * (2.0 * PI * fundamental * t).sin()
                    + 1.0 * (2.0 * PI * fundamental * 2.0 * t).sin()
                    + 0.2 * (2.0 * PI * fundamental * 3.0 * t).sin()
            })
            .collect();

        let estimate = estimate_pitch(&samples, sample_rate);
        let estimated_fundamental = estimate.fundamental_hz.expect("ピークが検出されるはず");
        let estimated_strongest = estimate.strongest_hz.expect("ピークが検出されるはず");

        let bin_width = sample_rate / num_samples as f32;
        assert!(
            (estimated_fundamental - fundamental).abs() < bin_width,
            "estimated {estimated_fundamental}Hz should be near the fundamental {fundamental}Hz, not a harmonic"
        );
        // 最強ピーク側は、意図通り2倍音(440Hz)を拾っているはず。
        assert!(
            (estimated_strongest - fundamental * 2.0).abs() < bin_width,
            "strongest {estimated_strongest}Hz should be near the 2nd harmonic {}Hz",
            fundamental * 2.0
        );
        assert!(
            estimate.strongest_strength.unwrap() > estimate.fundamental_strength.unwrap(),
            "the 2nd harmonic was made louder than the fundamental, so its strength should be greater"
        );
    }

    #[test]
    fn returns_none_for_silence() {
        let samples = vec![0.0f32; 4096];
        let estimate = estimate_pitch(&samples, 44100.0);
        assert_eq!(estimate.fundamental_hz, None);
        assert_eq!(estimate.fundamental_strength, None);
        assert_eq!(estimate.strongest_hz, None);
        assert_eq!(estimate.strongest_strength, None);
    }

    #[test]
    fn precision_at_gui_chunk_size_stays_within_a_few_cents() {
        // gui.rsの実際の設定(NUM_SAMPLES=1024)で、ギター低音弦(E2=82.4Hz)から
        // 高音域まで、実用上問題ないセント精度が出続けることを保証する回帰テスト。
        // Hann窓を外すなど精度を悪化させる変更をすると、ここで検知できる。
        const NUM_SAMPLES: usize = 1024;
        const MAX_ALLOWED_ERROR_CENTS: f32 = 15.0;

        for &sample_rate in &[44100.0f32, 48000.0] {
            for &frequency in &[82.4, 220.0, 440.0, 880.0, 1046.5, 2093.0, 4978.0] {
                let samples = fft::generate_sine_wave(frequency, sample_rate, NUM_SAMPLES);
                let f0 = estimate_pitch(&samples, sample_rate)
                    .fundamental_hz
                    .expect("ピークが検出されるはず");
                let error_cents = (1200.0 * (f0 / frequency).log2()).abs();

                assert!(
                    error_cents < MAX_ALLOWED_ERROR_CENTS,
                    "sr={sample_rate} f={frequency}Hz: error {error_cents:.2}cent exceeds {MAX_ALLOWED_ERROR_CENTS}cent"
                );
            }
        }
    }
}
