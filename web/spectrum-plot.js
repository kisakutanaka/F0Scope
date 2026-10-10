// gui.rs の spectrum_plot と同じ3系列を重ねて描く。x・y とも全データが収まるよう毎回自動スケールする。
const SERIES = [
  { key: 'coarse', label: '粗FFT', color: 'rgb(80, 140, 255)', lineWidth: 1.5 },
  { key: 'zoom', label: '局所FFT', color: 'rgb(230, 60, 60)', radius: 3 },
  { key: 'fitCurve', label: 'フィッティング', color: 'rgb(240, 210, 40)', lineWidth: 2.5 },
];
const AXIS_HEIGHT = 18;
const FONT = '11px system-ui, sans-serif';

export function drawSpectrum(canvas, analysis) {
  const dpr = window.devicePixelRatio || 1;
  const { clientWidth: w, clientHeight: h } = canvas;
  if (canvas.width !== Math.round(w * dpr)) canvas.width = Math.round(w * dpr);
  if (canvas.height !== Math.round(h * dpr)) canvas.height = Math.round(h * dpr);
  const g = canvas.getContext('2d');
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);

  const points = analysis ? SERIES.flatMap(({ key }) => analysis[key]) : [];
  if (points.length === 0) return;

  const [xMin, xMax] = paddedRange(points.map(([x]) => x));
  const [yMin, yMax] = paddedRange(points.map(([, y]) => y));
  const plotHeight = h - AXIS_HEIGHT;
  const sx = (x) => (x - xMin) / (xMax - xMin) * w;
  const sy = (y) => plotHeight - (y - yMin) / (yMax - yMin) * plotHeight;

  // x 軸の目盛り(Hz)
  g.font = FONT;
  g.textAlign = 'center';
  g.textBaseline = 'top';
  const step = niceStep((xMax - xMin) / 5);
  for (let x = Math.ceil(xMin / step) * step; x <= xMax; x += step) {
    g.fillStyle = 'rgba(255, 255, 255, 0.08)';
    g.fillRect(sx(x), 0, 1, plotHeight);
    g.fillStyle = 'rgba(255, 255, 255, 0.5)';
    g.fillText(`${+x.toPrecision(6)}`, sx(x), plotHeight + 4);
  }

  for (const { key, color, lineWidth, radius } of SERIES) {
    g.strokeStyle = g.fillStyle = color;
    if (radius) {
      for (const [x, y] of analysis[key]) {
        g.beginPath();
        g.arc(sx(x), sy(y), radius, 0, 2 * Math.PI);
        g.fill();
      }
    } else {
      g.lineWidth = lineWidth;
      g.beginPath();
      for (const [x, y] of analysis[key]) g.lineTo(sx(x), sy(y));
      g.stroke();
    }
  }

  // 凡例
  g.textAlign = 'left';
  g.textBaseline = 'middle';
  SERIES.forEach(({ label, color }, i) => {
    g.fillStyle = color;
    g.fillRect(w - 110, 10 + i * 16, 10, 3);
    g.fillStyle = 'rgba(255, 255, 255, 0.8)';
    g.fillText(label, w - 94, 11 + i * 16);
  });
}

function paddedRange(values) {
  const min = Math.min(...values);
  const max = Math.max(...values);
  const pad = (max - min) * 0.05 || 1;
  return [min - pad, max + pad];
}

// 1, 2, 5 × 10^n のうち rough 以上で最小の値。
function niceStep(rough) {
  const base = 10 ** Math.floor(Math.log10(rough));
  return [1, 2, 5, 10].map((m) => m * base).find((s) => s >= rough);
}
