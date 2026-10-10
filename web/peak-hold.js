// gui.rs の PeakHold と同じ。直近 holdMs ミリ秒に届いた読みのうち、最も強かったものを保持する。
// 時刻 now(ミリ秒)は呼び出し側から渡す(テストで時刻を注入するため)。
export class PeakHold {
  #history = [];

  constructor(holdMs) {
    this.holdMs = holdMs;
  }

  push(reading, now) {
    if (reading) this.#history.push({ t: now, reading });
    this.evictExpired(now);
  }

  setHoldDuration(holdMs, now) {
    this.holdMs = holdMs;
    this.evictExpired(now);
  }

  evictExpired(now) {
    while (this.#history.length > 0 && now - this.#history[0].t > this.holdMs) this.#history.shift();
  }

  // 同じ強さなら後に入ったものを返す(Rust の max_by と同じ)。
  peak() {
    let best = null;
    for (const { reading } of this.#history) {
      if (!best || reading.strength >= best.strength) best = reading;
    }
    return best;
  }
}
