import init, { analyze, empty_reading_text } from './pkg/pitch_core.js';

await init();

// 動作確認用: 440Hz のサイン波 1024 サンプルを解析して表示する。
const sampleRate = 48000;
const samples = Float32Array.from({ length: 1024 }, (_, i) => Math.sin(2 * Math.PI * 440 * i / sampleRate));
const { fundamental, strongest } = analyze(samples, sampleRate, 440);
const text = (reading) => reading ? reading.readingText : empty_reading_text();

document.getElementById('output').textContent =
  `基音 (F0): ${text(fundamental)}\n最大ピーク: ${text(strongest)}`;
