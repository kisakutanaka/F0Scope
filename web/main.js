import init, { analyze, empty_reading_text } from './pkg/pitch_core.js';
import { PeakHold } from './peak-hold.js';
import { drawSpectrum } from './spectrum-plot.js';

await init();

const DEFAULT_REFERENCE_A4_HZ = 440.0;
const DEFAULT_HOLD_SEC = 1.0;
const EMPTY_READING = empty_reading_text();
const $ = (id) => document.getElementById(id);
const debug = new URLSearchParams(location.search).get('debug') === '1';

let referenceA4Hz = DEFAULT_REFERENCE_A4_HZ;
let ctx;
let latest = null;
const fundamentalHold = new PeakHold(DEFAULT_HOLD_SEC * 1000);
const strongestHold = new PeakHold(DEFAULT_HOLD_SEC * 1000);
const laps = [];

// ?debug=1 のときの計測(「同じ速度」の検証用)。1秒ごとに集計する。
let chunkCount = 0;
let maxAnalysisMs = 0;
let statsStart = 0;

$('start').addEventListener('click', async () => {
  $('start').disabled = true;
  try {
    await start();
    $('start').hidden = true;
    $('error').hidden = true;
  } catch (e) {
    $('error').textContent = `マイクを開始できませんでした: ${e.message}`;
    $('error').hidden = false;
    $('start').disabled = false;
  }
});

async function start() {
  const stream = await navigator.mediaDevices.getUserMedia({
    // ブラウザの音声処理は持続音を削るなどしてピッチ検出を壊すので、すべて切る。
    audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false },
  });
  ctx = new AudioContext(); // sampleRate は強制しない(リサンプリングを避ける)
  await ctx.audioWorklet.addModule('./capture-processor.js');
  const node = new AudioWorkletNode(ctx, 'capture-processor');
  ctx.createMediaStreamSource(stream).connect(node);
  node.connect(ctx.destination); // 処理を回すため。出力は無音
  node.port.onmessage = ({ data }) => {
    const t0 = performance.now();
    latest = analyze(data, ctx.sampleRate, referenceA4Hz);
    const now = performance.now();
    fundamentalHold.push(latest.fundamental, now);
    strongestHold.push(latest.strongest, now);
    maxAnalysisMs = Math.max(maxAnalysisMs, now - t0);
    chunkCount++;
  };
  await ctx.resume();
  $('debug').hidden = !debug;
  statsStart = performance.now();
}

// --- 操作 ---

function setReference(hz) {
  referenceA4Hz = Math.min(466, Math.max(415, Math.round(hz * 10) / 10));
  $('reference').value = referenceA4Hz;
  $('reference-value').textContent = `${referenceA4Hz.toFixed(1)} Hz`;
}
$('reference').addEventListener('input', (e) => setReference(Number(e.target.value)));
$('reference-reset').addEventListener('click', () => setReference(DEFAULT_REFERENCE_A4_HZ));
for (const button of document.querySelectorAll('[data-delta]')) {
  button.addEventListener('click', () => setReference(referenceA4Hz + Number(button.dataset.delta)));
}

$('hold').addEventListener('input', (e) => {
  const sec = Number(e.target.value);
  $('hold-value').textContent = `${sec.toFixed(1)} 秒`;
  const now = performance.now();
  fundamentalHold.setHoldDuration(sec * 1000, now);
  strongestHold.setHoldDuration(sec * 1000, now);
});

// ラップはホールド値ではなく最新値を記録する(gui.rs と同じ)。
function recordLap() {
  laps.push({
    fundamental: readingText(latest?.fundamental),
    strongest: readingText(latest?.strongest),
  });
  $('lap').textContent = `ラップ記録 (${laps.length}件)`;
  $('laps').textContent = laps
    .map((lap, i) => `#${String(i + 1).padStart(2)}  基音: ${lap.fundamental}\n     最大: ${lap.strongest}`)
    .reverse()
    .join('\n');
}
$('lap').addEventListener('click', recordLap);
document.addEventListener('keydown', (e) => {
  if (e.code !== 'Space') return;
  e.preventDefault(); // ページのスクロールやボタンの押下を防ぐ
  recordLap();
});

// --- 描画 ---

function readingText(reading) {
  return reading ? reading.readingText : EMPTY_READING;
}

// スマホ幅に収まるよう、音名・セント(pitchText)と周波数を2行に分ける。
// 周波数は Rust で整形済みの readingText から切り出す(JS で丸め直さない)。
function showReading(id, reading, hold) {
  $(id).textContent = reading ? reading.pitchText : EMPTY_READING.trimEnd();
  const frequency = reading ? reading.readingText.slice(reading.pitchText.length).trim() : '';
  $(`${id}-detail`).textContent = `${frequency}\n└ ホールド: ${readingText(hold.peak())}`;
}

function render(now) {
  // 新しい読みが来ないフレームでも、保持時間切れのピークは掃除する(gui.rs と同じ)。
  fundamentalHold.evictExpired(now);
  strongestHold.evictExpired(now);
  showReading('fundamental', latest?.fundamental, fundamentalHold);
  showReading('strongest', latest?.strongest, strongestHold);
  drawSpectrum($('spectrum'), latest);

  if (debug && ctx && now - statsStart >= 1000) {
    const rate = chunkCount * 1000 / (now - statsStart);
    $('debug').textContent = `${rate.toFixed(1)} 回/秒  解析 最大 ${maxAnalysisMs.toFixed(2)} ms  ${ctx.sampleRate} Hz`;
    chunkCount = 0;
    maxAnalysisMs = 0;
    statsStart = now;
  }
  requestAnimationFrame(render);
}
requestAnimationFrame(render);
