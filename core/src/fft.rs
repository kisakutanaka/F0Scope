use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

/// 音声コールバックから届く可変長のサンプル列を蓄積し、`capacity`個貯まるごとに
/// FFT1回分のチャンクを取り出せるようにするバッファ。
pub struct SampleBuffer {
    samples: Vec<f32>,
    capacity: usize,
}

impl SampleBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// 新しいサンプルを追加する。`capacity`個以上溜まったら、先頭から`capacity`個を
    /// 取り出して`Some`で返す(取り出した分は内部バッファから取り除かれる)。
    /// まだ足りなければ`None`を返す。
    pub fn push(&mut self, data: &[f32]) -> Option<Vec<f32>> {
        self.samples.extend_from_slice(data);
        if self.samples.len() >= self.capacity {
            Some(self.samples.drain(..self.capacity).collect())
        } else {
            None
        }
    }
}

/// 指定周波数・サンプルレートのサイン波を`num_samples`個生成する。
pub fn generate_sine_wave(frequency: f32, sample_rate: f32, num_samples: usize) -> Vec<f32> {
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate;
            (2.0 * std::f32::consts::PI * frequency * t).sin()
        })
        .collect()
}

/// サンプル列にHann窓をかけたものを返す(元の配列は変更しない)。
/// 矩形窓のまま(何もしない)FFTだと、解析窓の端で波形が不連続になり、
/// 本来1本のはずのピークが周囲のビンへ滲み出す(スペクトル漏れ)。
/// 特に解析窓内に収まる波の周期数が少ない低い周波数ほど影響が大きく、
/// ピーク位置の推定精度が落ちる。Hann窓は両端がなめらかに0へ近づく形をしており、
/// この不連続を無くして漏れを大きく減らす。
pub fn hann_window(samples: &[f32]) -> Vec<f32> {
    let n = samples.len();
    if n <= 1 {
        return samples.to_vec();
    }

    samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let phase = 2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32;
            let coefficient = 0.5 - 0.5 * phase.cos();
            s * coefficient
        })
        .collect()
}

/// 実数サンプル列にFFTをかけ、各周波数ビンの振幅(マグニチュード)を返す。
pub fn magnitude_spectrum(samples: &[f32]) -> Vec<f32> {
    let mut buffer: Vec<Complex<f32>> = samples.iter().map(|&s| Complex::new(s, 0.0)).collect();

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(buffer.len());
    fft.process(&mut buffer);

    buffer.iter().map(|c| c.norm()).collect()
}

/// マグニチュードスペクトルから極大値(隣接ビンより強いビン)を複数検出し、
/// 強い順に上位`max_candidates`個を(周波数[Hz], 振幅)のペアとして、周波数の昇順で返す。
/// 単一の最大値だけを拾う方式だと倍音を基本周波数と誤認しやすいため、
/// 複数の候補を残して呼び出し側(基本周波数の選定ロジック)に判断を委ねる。
/// 実数信号のスペクトルは前半と後半が対称になるため、前半(ナイキスト周波数以下)のみを走査する。
/// ビン0(直流成分)はどんな信号でも大きくなりがちなので除外する。
pub fn find_peak_candidates(
    magnitudes: &[f32],
    sample_rate: f32,
    max_candidates: usize,
) -> Vec<(f32, f32)> {
    let half = magnitudes.len() / 2;
    let bin_width = sample_rate / magnitudes.len() as f32;

    let mut local_maxima: Vec<(usize, f32)> = (1..half.saturating_sub(1))
        .filter(|&i| magnitudes[i] > magnitudes[i - 1] && magnitudes[i] > magnitudes[i + 1])
        .map(|i| (i, magnitudes[i]))
        .collect();

    local_maxima.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    local_maxima.truncate(max_candidates);
    local_maxima.sort_by(|a, b| a.0.cmp(&b.0));

    local_maxima
        .into_iter()
        .map(|(bin, magnitude)| (bin as f32 * bin_width, magnitude))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_sine_wave_frequency() {
        let sample_rate = 44100.0;
        let num_samples = 4096;
        let frequency = 440.0;

        let samples = generate_sine_wave(frequency, sample_rate, num_samples);
        let magnitudes = magnitude_spectrum(&samples);
        let candidates = find_peak_candidates(&magnitudes, sample_rate, 5);

        let bin_width = sample_rate / num_samples as f32;
        let (peak, _) = candidates
            .iter()
            .cloned()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .expect("候補が見つかりません");
        assert!(
            (peak - frequency).abs() <= bin_width,
            "peak {peak} was not within one bin ({bin_width}Hz) of {frequency}"
        );
    }

    #[test]
    fn hann_window_tapers_edges_to_zero_and_peaks_at_center() {
        let samples = vec![1.0f32; 9];
        let windowed = hann_window(&samples);

        assert_eq!(windowed.len(), samples.len());
        assert!(windowed[0].abs() < 1e-6, "first sample should taper to ~0");
        assert!(windowed[8].abs() < 1e-6, "last sample should taper to ~0");
        assert!(
            (windowed[4] - 1.0).abs() < 1e-6,
            "center sample should be left ~unchanged"
        );
    }
}
