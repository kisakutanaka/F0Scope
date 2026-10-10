use std::fmt;

/// 基準ピッチのデフォルト値(A4 = 440Hz)。実行時に別の値へ変更可能。
pub const DEFAULT_REFERENCE_A4_HZ: f32 = 440.0;

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
/// MIDIノート番号におけるA4(基準ピッチが指す音)の番号。
const A4_MIDI_NUMBER: i32 = 69;

/// 周波数を、最も近い十二平均律の音(ノート名+オクターブ)とそこからのズレ(セント)で表したもの。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pitch {
    pub note_name: &'static str,
    pub octave: i32,
    /// 最も近い音からのズレ。-50.0〜+50.0セントの範囲に収まる。
    pub cents: f32,
}

impl fmt::Display for Pitch {
    /// 表示中に音名やセントの桁数が変わってガタつかないよう、各項目を固定幅で揃える。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<2}{:>2} {:>+4.0}cent",
            self.note_name, self.octave, self.cents
        )
    }
}

/// 周波数を、指定した基準ピッチ(A4の周波数)をもとにノート名+セント偏差へ変換する。
pub fn frequency_to_pitch(frequency: f32, reference_a4_hz: f32) -> Pitch {
    let semitones_from_a4 = 12.0 * (frequency / reference_a4_hz).log2();
    let nearest_semitone = semitones_from_a4.round();
    let cents = (semitones_from_a4 - nearest_semitone) * 100.0;

    let midi_number = A4_MIDI_NUMBER + nearest_semitone as i32;
    let pitch_class = midi_number.rem_euclid(12) as usize;
    let octave = midi_number.div_euclid(12) - 1;

    Pitch {
        note_name: NOTE_NAMES[pitch_class],
        octave,
        cents,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_matches_reference_exactly() {
        let pitch = frequency_to_pitch(440.0, DEFAULT_REFERENCE_A4_HZ);
        assert_eq!(pitch.note_name, "A");
        assert_eq!(pitch.octave, 4);
        assert!(pitch.cents.abs() < 0.01);
    }

    #[test]
    fn a_sharp4_is_detected() {
        let pitch = frequency_to_pitch(466.16, DEFAULT_REFERENCE_A4_HZ);
        assert_eq!(pitch.note_name, "A#");
        assert_eq!(pitch.octave, 4);
        assert!(pitch.cents.abs() < 1.0);
    }

    #[test]
    fn sharp_frequency_reports_positive_cents() {
        // 450Hzは A4(440Hz) より約39セント高い。
        let pitch = frequency_to_pitch(450.0, DEFAULT_REFERENCE_A4_HZ);
        assert_eq!(pitch.note_name, "A");
        assert_eq!(pitch.octave, 4);
        assert!((pitch.cents - 39.0).abs() < 1.0, "cents was {}", pitch.cents);
    }

    #[test]
    fn reference_pitch_is_adjustable() {
        // 基準ピッチをA4=442Hzに変えても、442Hz自体はズレ0のA4として検出される。
        let pitch = frequency_to_pitch(442.0, 442.0);
        assert_eq!(pitch.note_name, "A");
        assert_eq!(pitch.octave, 4);
        assert!(pitch.cents.abs() < 0.01);
    }
}
