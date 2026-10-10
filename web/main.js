import init, { analyze, empty_reading_text } from './pkg/pitch_core.js';

await init();

const EMPTY_READING = empty_reading_text();
const startButton = document.getElementById('start');
const output = document.getElementById('output');
const debugOutput = document.getElementById('debug');
const debug = new URLSearchParams(location.search).get('debug') === '1';

let referenceA4Hz = 440.0;
let ctx;
let latest = null;

// ?debug=1 のときの計測(「同じ速度」の検証用)。1秒ごとに集計する。
let chunkCount = 0;
let maxAnalysisMs = 0;
let statsStart = 0;

startButton.addEventListener('click', async () => {
  startButton.disabled = true;
  try {
    await start();
    startButton.hidden = true;
  } catch (e) {
    output.textContent = `マイクを開始できませんでした: ${e.message}`;
    startButton.disabled = false;
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
    maxAnalysisMs = Math.max(maxAnalysisMs, performance.now() - t0);
    chunkCount++;
  };
  await ctx.resume();
  debugOutput.hidden = !debug;
  statsStart = performance.now();
  requestAnimationFrame(render);
}

function render(now) {
  const text = (reading) => reading ? reading.readingText : EMPTY_READING;
  output.textContent = `基音 (F0): ${text(latest?.fundamental)}\n最大ピーク: ${text(latest?.strongest)}`;

  if (debug && now - statsStart >= 1000) {
    const rate = chunkCount * 1000 / (now - statsStart);
    debugOutput.textContent = `${rate.toFixed(1)} 回/秒  解析 最大 ${maxAnalysisMs.toFixed(2)} ms  ${ctx.sampleRate} Hz`;
    chunkCount = 0;
    maxAnalysisMs = 0;
    statsStart = now;
  }
  requestAnimationFrame(render);
}
