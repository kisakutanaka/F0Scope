// デスクトップ版の NUM_SAMPLES と同じ。精度・更新頻度が変わるので変更しないこと。
const CHUNK_SIZE = 1024;

// マイク入力をモノラル化して CHUNK_SIZE 個ずつ、重なりなし・抜けなしでメインスレッドへ送る
// (fft::SampleBuffer と audio.rs のモノラル化と同じ)。
class CaptureProcessor extends AudioWorkletProcessor {
  buffer = new Float32Array(CHUNK_SIZE);
  filled = 0;

  process([channels]) {
    if (channels.length === 0) return true;
    for (let i = 0; i < channels[0].length; i++) {
      let sum = 0;
      for (const channel of channels) sum += channel[i];
      this.buffer[this.filled++] = sum / channels.length;
      if (this.filled === CHUNK_SIZE) {
        this.port.postMessage(this.buffer, [this.buffer.buffer]);
        this.buffer = new Float32Array(CHUNK_SIZE);
        this.filled = 0;
      }
    }
    return true;
  }
}

registerProcessor('capture-processor', CaptureProcessor);
