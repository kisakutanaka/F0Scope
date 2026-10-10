# F0 Scope（GitHub Pages版 音程検出アプリ）実装計画（Claude Code向け引き継ぎ資料）

| 項目 | 値 |
|---|---|
| アプリ表示名 | F0 Scope（`<title>`、ページ見出し、PWA の `name`/`short_name`） |
| リポジトリ名 | `F0Scope` |
| ローカル作業ディレクトリ | `/Users/tanakakisaku/Documents/dev/tools/F0Scope` |
| リモート | `git@github.com:kisakutanaka/F0Scope.git`（GitHub 上に作成済み・空） |
| 公開 URL | `https://kisakutanaka.github.io/F0Scope/` |
| デフォルトブランチ | `main`（デプロイ workflow のトリガー） |

デスクトップ版（Rust + cpal + egui）の音程検出アプリ `FFTexample` を、**スマホのブラウザから使える GitHub Pages 版**として作り直すための引き継ぎ資料。新しい作業ディレクトリで Claude Code がこれを読み、ステップ順に実装することを想定している。ユーザーとの会話は日本語で行う。

> **最初にやること**: このファイルを最後まで読む → 「9. ステップ一覧」の Step 1 から始める。Step 1 の中で、このファイルを元に新リポジトリ用の `CLAUDE.md` を作る。

---

## 1. 目的と「同じ」の定義

ユーザーの要望は「**全く同じ精度と速度**」。ここでは次の3点をすべて満たすことを「同じ」と定義する。

| 観点 | 要件 |
|---|---|
| 精度 | 同じサンプル列・サンプルレートを入れたら、デスクトップ版と同じ基音/最大ピーク周波数（差 ±0.05 cent 以内）、同じ音名・オクターブが出る。→ **アルゴリズムの Rust コードをそのまま WASM にコンパイルして使う**ことで担保する（JS/TS への書き直しは禁止） |
| 速度（更新頻度・遅延） | 1回の推定に使うサンプル数 1024、重なりなし・抜けなしの連続チャンク。更新間隔 = 1024 / サンプルレート（48kHz で約 21.3ms ≒ 47回/秒、44.1kHz で約 23.2ms ≒ 43回/秒） |
| 計算時間 | デスクトップ版の解析1回は約 0.12ms（Apple Silicon・release ビルド実測）。WASM ではスマホでも **1回 2ms 以下**を目標とする（更新間隔 21ms に対して十分な余裕） |
| 挙動 | 基音選定・ピークホールド・ラップ・基準ピッチ変更の振る舞いがデスクトップ版と同じ。無音判定やスムージングなど、元にない処理を勝手に足さない |

UI の見た目はスマホ向けに作り直してよい（egui をそのまま持ち込む必要はない）。ただし表示する文字列の中身は同じにする（後述: 文字列整形も Rust 側で行う）。

---

## 2. 移植元

移植元リポジトリ: `/Users/tanakakisaku/Documents/dev/rust/FFTexample`（同じ Mac 上。読み取りのみ。移植元は変更しない）

| 移植元ファイル | 扱い |
|---|---|
| `src/fft.rs` | **無改変でコピー**（`SampleBuffer`, `hann_window`, `magnitude_spectrum`, `find_peak_candidates`, `generate_sine_wave`） |
| `src/pitch.rs` | **無改変でコピー**（`frequency_to_pitch`, `Pitch` の `Display`） |
| `src/pitch_detect.rs` | **無改変でコピー**（粗FFT→候補検出→基音選定→局所DTFT→放物線補間。テスト込み） |
| `src/gui.rs` | コピーしない。仕様の参照元。`format_reading`/`READING_WIDTH` だけは `lib.rs` に移植する。`PeakHold`・ラップ・UI は JS で再実装する |
| `src/audio.rs` | コピーしない。cpal はブラウザのマイク入力に使えないため、Web Audio API（AudioWorklet）で置き換える。モノラル化の仕様だけ踏襲する |
| `src/meter.rs`, `src/main.rs` | 不要 |

コピーした3ファイルは `crate::fft` を参照しているので、`lib.rs` でクレートのルートに `mod fft; mod pitch; mod pitch_detect;` を置けば無改変でコンパイルできる。今後デスクトップ版のアルゴリズムが更新されたら、この3ファイルを再コピーするだけで追従できる状態を保つこと。

---

## 3. アーキテクチャの決定

```
マイク(getUserMedia, 音声処理OFF)
  → AudioWorklet(オーディオスレッド): 128フレームずつ届く入力をモノラル化して1024個ずつ貯め、postMessage
  → メインスレッド: WASM の analyze(samples, sampleRate, refA4) を呼ぶ（中身はデスクトップ版と同じ Rust コード）
  → JS: PeakHold 更新・ラップ記録
  → requestAnimationFrame で DOM と Canvas(スペクトル) を描画
```

**採用: Rust コア → WASM（wasm-bindgen）+ 素の HTML/CSS/JS（バンドラなし）**

- 精度の担保: 移植元の Rust コードを無改変で使う。f32 演算もそのまま
- 文字列整形も Rust 側: `Pitch` の `Display`（`{:<2}{:>2} {:>+4.0}cent`）や `format_reading` を JS で書き直すと、丸め方や `-0` の扱い（Rust は `-0cent` を出す）が変わる。WASM から整形済み文字列を返す
- バンドラなし: `wasm-pack build --target web` の出力を ES Modules で直接読む。Node/npm がなくてもビルドでき、GitHub Pages には静的ファイルを置くだけ

**採用しなかった案**

- eframe の Web 版で egui UI ごと移植: cpal は Web でマイク入力に対応していないため、音声入力は結局 web-sys で書く必要がある。加えて日本語フォントを数MB同梱する必要があり、スマホでの操作性も悪い
- TS でアルゴリズムを書き直す: 「同じ精度」を検証し続けるコストが高く、f64/f32 の違いで微妙にずれる
- WASM を AudioWorklet 内で動かす: AudioWorkletGlobalScope では wasm-bindgen の生成コード（TextDecoder 等）がそのままでは動かず、構成が複雑になる。解析1回は 1ms 程度なのでメインスレッドで十分

---

## 4. ディレクトリ構成（新リポジトリ）

```
F0Scope/
├── CLAUDE.md                  # Step 1 で作成
├── WEB_PORT_PLAN.md           # このファイル
├── core/                      # Rust クレート（WASM化するコア）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs             # wasm-bindgen の公開API + format_reading
│       ├── fft.rs             # 移植元から無改変コピー
│       ├── pitch.rs           # 移植元から無改変コピー
│       └── pitch_detect.rs    # 移植元から無改変コピー
├── web/                       # GitHub Pages で公開するディレクトリ
│   ├── index.html
│   ├── style.css
│   ├── main.js                # 起動・音声・描画
│   ├── capture-processor.js   # AudioWorkletProcessor（別ファイル必須）
│   ├── peak-hold.js           # PeakHold の移植
│   ├── spectrum-plot.js       # Canvas によるスペクトル描画
│   └── pkg/                   # wasm-pack の出力（.gitignore 対象、CIで生成）
├── tests/
│   └── peak-hold.test.js      # node --test で実行
└── .github/workflows/deploy.yml
```

GitHub Pages ではリポジトリ名のサブパス（`https://kisakutanaka.github.io/F0Scope/`）で配信されるので、**HTML/JS 内の参照はすべて相対パス**（`./pkg/...`, `./capture-processor.js`）にする。

---

## 5. 変更禁止パラメータ（精度・速度を決める値）

これらを変えると「同じ」でなくなる。変更はユーザーの明示的な指示があるときだけ。

| 値 | 場所 | 意味 |
|---|---|---|
| `NUM_SAMPLES = 1024` | gui.rs → JS の `CHUNK_SIZE` | 1回の解析のサンプル数。重なりなし、連続チャンク |
| `COARSE_CANDIDATES = 5` | pitch_detect.rs | 粗FFTの極大値候補数 |
| `FUNDAMENTAL_THRESHOLD_RATIO = 0.2` | pitch_detect.rs | 最強候補の20%以上の振幅を持つ候補のうち最低周波数を基音とする |
| `ZOOM_SEARCH_WIDTH_IN_BINS = 1.0`, `ZOOM_SEARCH_POINTS = 21` | pitch_detect.rs | 局所DTFTの探索幅（±1ビン）と点数 |
| `COARSE_SPECTRUM_DISPLAY_MAX_HZ = 5000` | pitch_detect.rs | グラフに送る粗FFTの上限周波数 |
| Hann 窓 | fft.rs | 粗FFT・局所DTFTの両方に同じ窓をかけた信号を使う |
| サンプルレート | AudioContext の既定値 | デバイス本来のレート。`new AudioContext({sampleRate})` で**強制しない**。解析には `ctx.sampleRate` を渡す |
| 基準ピッチ A4 | 既定 440.0、範囲 415.0〜466.0、刻み 0.1 | gui.rs と同じ |
| ピークホールド時間 | 既定 1.0秒、範囲 0.2〜5.0秒、刻み 0.1 | gui.rs と同じ |

コア3ファイル内の定数は無改変コピーなので自動的に守られる。JS 側の `CHUNK_SIZE` とスライダー範囲は手で合わせること。

---

## 6. 実装仕様

### 6.1 core（Rust → WASM）

`core/Cargo.toml`（バージョンは実装時点の最新安定版を確認して使う。rustfft は移植元と同じ 6.4.1 に固定）:

```toml
[package]
name = "pitch_core"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
rustfft = "=6.4.1"
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde-wasm-bindgen = "0.6"

[dev-dependencies]
wasm-bindgen-test = "0.3"

[profile.release]
opt-level = 3      # 速度優先。"s"/"z" にしない
```

`core/src/lib.rs` の公開API（イメージ。コンパイルが通るよう細部は調整してよい）:

```rust
mod fft;
mod pitch;
mod pitch_detect;

use serde::Serialize;
use wasm_bindgen::prelude::*;

/// gui.rs の READING_WIDTH / format_reading をそのまま移植(26文字固定幅)。
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
    pitch_text: String,   // Pitch の Display（13文字固定。例 "A  4   +0cent"）
    reading_text: String, // format_reading の結果（26文字固定。ホールド・ラップ表示用）
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Analysis {
    fundamental: Option<Reading>,
    strongest: Option<Reading>,
    coarse: Vec<(f32, f32)>,    // [周波数, 振幅] の配列
    zoom: Vec<(f32, f32)>,
    fit_curve: Vec<(f32, f32)>,
}

/// gui.rs のオーディオコールバック内の処理(analyze_with_spectrum → frequency_to_pitch)と同じことをする。
#[wasm_bindgen]
pub fn analyze(samples: &[f32], sample_rate: f32, reference_a4_hz: f32) -> Result<JsValue, JsValue> { ... }

/// format_reading(None)。ラップ記録時に読みが無い場合に使う。
#[wasm_bindgen]
pub fn empty_reading_text() -> String { format_reading(None) }
```

ポイント:
- 戻り値は `serde-wasm-bindgen` でプレーンな JS オブジェクトにする。`#[wasm_bindgen]` 構造体を返すと JS 側で `.free()` を呼ばないとメモリリークするため避ける
- `std::time::Instant` は `wasm32-unknown-unknown` で**実行時に panic する**。コア3ファイルは使っていないが、lib.rs にも持ち込まないこと（時間管理は JS 側）
- `gui.rs` の `format_reading_width_is_constant_regardless_of_digits` テストを lib.rs に移植する
- 開発中は `console_error_panic_hook` を入れておくと WASM 内の panic が読める（任意）

### 6.2 音声入力

```js
const stream = await navigator.mediaDevices.getUserMedia({
  audio: {
    echoCancellation: false,   // 必須: ブラウザの音声処理はすべて切る
    noiseSuppression: false,   // 必須: ONだと持続音がノイズとして削られ、ピッチ検出が壊れる
    autoGainControl: false,    // 必須
  },
});
const ctx = new AudioContext();          // sampleRate は指定しない
await ctx.audioWorklet.addModule('./capture-processor.js');
const source = ctx.createMediaStreamSource(stream);
const node = new AudioWorkletNode(ctx, 'capture-processor');
source.connect(node);
node.connect(ctx.destination);           // 処理を回すため接続する。出力には何も書かないので無音
```

`capture-processor.js` の仕様（`fft::SampleBuffer` と `audio.rs` のモノラル化を踏襲）:
- `inputs[0]` が空（まだ入力がない）のときは何もせず `return true`
- 1チャンネルならそのまま、複数チャンネルなら各フレームでチャンネル平均してモノラル化
- `Float32Array(1024)` に詰め、満杯になったら `this.port.postMessage(buf, [buf.buffer])` で転送し、新しいバッファで続ける。**重なりなし・抜けなし**（SampleBuffer と同じ）
- 出力配列には何も書かない、常に `return true`

メインスレッド側:
- `node.port.onmessage` で受けたら即 `analyze(e.data, ctx.sampleRate, currentRefA4)` を呼ぶ。基準ピッチは**呼び出し時点の値**を渡す（デスクトップ版で AtomicU32 から読んでいるのと同じ）
- 結果を `latest` に保存し、`fundamental`/`strongest` を各 PeakHold に push する（gui.rs で受信のたびに push しているのと同じ）
- DOM 更新は `requestAnimationFrame` ループで行う（解析ごとに DOM を触らない）
- 開始は必ずユーザー操作（「開始」ボタン）から。iOS Safari はユーザー操作なしに AudioContext を動かせない
- `?debug=1` のときだけ、実測の更新頻度（回/秒）、解析1回の時間（`performance.now()` で計測した ms）、`ctx.sampleRate` を小さく表示する。「同じ速度」の検証用

### 6.3 UI（gui.rs と同じ要素）

上から順に:

1. **基準ピッチ A4**: スライダー（415.0〜466.0、step 0.1、表示 `440.0 Hz`）＋「440Hzに戻す」ボタン。スマホでは指で 0.1Hz 単位を合わせにくいので、`−1` `−0.1` `+0.1` `+1` ボタンを追加する（範囲・刻みは同じ）
2. **ピークホールド時間**: スライダー（0.2〜5.0秒、step 0.1、表示 `1.0 秒`）
3. **基音 (F0)**: 最新の読みを大きく表示＋その下に `└ ホールド: <readingText>`
4. **最大ピーク**: 最新の読みを中くらいで表示＋その下に `└ ホールド: <readingText>`
5. **ラップ**: 「ラップ記録 (N件)」ボタン（スマホ用）。PC では Space キーでも記録できる（`preventDefault` してページスクロール・ボタン押下を防ぐ）。記録する値は**ホールド値ではなく最新値**（`latest`）。一覧は新しい順、高さ 120px 程度のスクロール領域、書式は gui.rs と同じ `#{index:>2}  基音: {readingText}  最大: {readingText}`（index は `padStart(2)`、読みが無いときは `empty_reading_text()`）
6. **スペクトル**: 粗FFT（線, rgb(80,140,255)）、局所FFT（点, rgb(230,60,60), 半径3）、フィッティング（線, rgb(240,210,40), 太さ2.5）を重ね描き。凡例付き、高さ 220px 程度。x・y とも全データが収まるように毎フレーム自動スケール（egui_plot の既定と同じ）

文字表示の注意:
- 読み（`pitchText` / `readingText`）は ASCII の空白で桁揃えされているので、**等幅フォント**（`ui-monospace, Menlo, monospace`）＋ **`white-space: pre`** で表示する。`pre` がないと空白が潰れて桁がずれる
- 26文字の `readingText` は 40px だとスマホ幅に収まらない。基音のメイン表示は `pitchText`（13文字）を大きく、周波数をその下に小さく表示するなど2行に分けてよい。文字サイズは `clamp()`/`vw` で調整し、折り返しや横スクロールが出ないようにする
- ダークテーマ基調（egui の既定と同じ）。`prefers-color-scheme` に合わせてもよい

`peak-hold.js`（gui.rs の `PeakHold` の移植）:
- `push(reading, now)`: `reading` があれば `{t: now, reading}` を末尾に追加し、その後 `evictExpired(now)`
- `evictExpired(now)`: 先頭から `now - t > holdMs` のものを捨てる（`>` であって `>=` ではない）
- `setHoldDuration(ms, now)`: 更新して `evictExpired`
- `peak()`: `strength` 最大の読み。**同値なら後に入ったものを返す**（Rust の `max_by` と同じ。比較は `>=`）
- 時刻は `performance.now()`。テストのため `now` は引数で注入できるようにする
- rAF の各フレームでも `evictExpired` を呼ぶ（無音時にホールドが自然に消えるように。gui.rs が毎フレーム呼んでいるのと同じ）
- gui.rs の2つのテスト（最強の読みを保持する／保持時間を過ぎたら忘れる）を `tests/peak-hold.test.js` に移植（`node --test` で実行。sleep せず `now` を注入）

### 6.4 モバイル対応

- GitHub Pages は HTTPS なので `getUserMedia` が使える（`http://` やファイル直開きでは不可。ローカル確認は `localhost`）
- iOS Safari: タブがバックグラウンドに行くと AudioContext が `suspended`/`interrupted` になる。`ctx.onstatechange` と `visibilitychange` を見て、止まっていたら「再開」ボタンを出す
- マイク許可が拒否された・非対応ブラウザの場合は、画面にわかりやすいエラーを出す
- 任意（Step 7）: Screen Wake Lock API で計測中に画面が消えないようにする／PWA マニフェストでホーム画面に追加できるようにする

---

## 7. 検証

### 7.1 コアのテスト

- `cargo test --manifest-path core/Cargo.toml`: コピーしたテスト（fft 2件、pitch 4件、pitch_detect 4件）＋移植した format_reading テストが全部通ること
- `wasm-pack test --node core`: 下の参照値テーブルを使った golden テストを `#[wasm_bindgen_test]` で書き、**WASM ビルドでも同じ値が出る**ことを確認する

### 7.2 参照値（デスクトップ版の実出力）

移植元コードで `fft::generate_sine_wave(f, sr, 1024)` を `pitch_detect::estimate_pitch` に通し、A4=440Hz で変換した値（2026-10-10 に移植元で実測）。

WASM 版の基音周波数がこの値と **±0.05 cent 以内**、音名・オクターブが一致することを確認する。CPU やライブラリの実装によって sin/cos や FFT の演算順序が違い末尾の桁がずれることがあるため、完全一致は求めない。セントが0付近の場合は `+0`/`-0` の符号が入れ替わることがあるので、セント部分の文字列は比較しない。

| サンプルレート | 入力 [Hz] | 推定基音 [Hz] | 真値との差 [cent] | 表示 | 基音の強さ |
|---|---|---|---|---|---|
| 44100 | 82.4 | 82.9313 | +11.13 | `E  2  +11cent` | 254.169 |
| 44100 | 110 | 110.2179 | +3.43 | `A  2   +3cent` | 225.126 |
| 44100 | 220 | 220.0177 | +0.14 | `A  3   +0cent` | 253.841 |
| 44100 | 440 | 439.9985 | -0.01 | `A  4   -0cent` | 248.109 |
| 44100 | 880 | 879.9984 | -0.00 | `A  5   -0cent` | 226.266 |
| 44100 | 1046.5 | 1046.5000 | 0.00 | `C  6   -0cent` | 241.304 |
| 44100 | 2093 | 2093.0002 | 0.00 | `C  7   -0cent` | 230.397 |
| 44100 | 4978 | 4978.0010 | 0.00 | `D# 8   -0cent` | 229.118 |
| 48000 | 82.4 | 82.4104 | +0.22 | `E  2   +0cent` | 246.201 |
| 48000 | 110 | 110.1514 | +2.38 | `A  2   +2cent` | 236.055 |
| 48000 | 220 | 220.0055 | +0.04 | `A  3   +0cent` | 240.668 |
| 48000 | 440 | 440.0045 | +0.02 | `A  4   +0cent` | 232.059 |
| 48000 | 880 | 880.0021 | 0.00 | `A  5   +0cent` | 247.404 |
| 48000 | 1046.5 | 1046.4977 | -0.00 | `C  6   -0cent` | 238.795 |
| 48000 | 2093 | 2093.0002 | 0.00 | `C  7   -0cent` | 236.283 |
| 48000 | 4978 | 4978.0000 | 0.00 | `D# 8   -0cent` | 249.404 |

倍音のほうが強い信号（48000Hz, 1024サンプル, `0.3·sin(220Hz) + 1.0·sin(440Hz) + 0.2·sin(660Hz)`）:
`fundamental_hz = 220.53801`, `fundamental_strength = 72.759995`, `strongest_hz = 439.9929`, `strongest_strength = 232.18135`

スナップショットの点数（48000Hz, 440Hz 正弦波）: `coarse = 107`, `zoom = 21`, `fit_curve = 21`

`frequency_to_pitch` の表示文字列:

| A4 基準 | 入力 [Hz] | 表示 |
|---|---|---|
| 440 | 440 | `A  4   +0cent` |
| 440 | 450 | `A  4  +39cent` |
| 440 | 466.16 | `A# 4   -0cent` |
| 440 | 82.4 | `E  2   -0cent` |
| 440 | 27.5 | `A  0   +0cent` |
| 440 | 4186 | `C  8   -0cent` |
| 442 | 440 | `A  4   -8cent` |
| 442 | 450 | `A  4  +31cent` |

補足: 44.1kHz・1024サンプルでは低音（E2 で +11cent、A2 で +3cent）の誤差が大きいが、**これはデスクトップ版と同じ特性**であり、移植で直すものではない（改善はユーザーと相談のうえ、デスクトップ版と同時に行う）。

### 7.3 実機確認（ユーザーに依頼する）

- iPhone（Safari）と Android（Chrome）で開き、チューナーアプリやトーンジェネレーターの 440Hz / 82.4Hz を鳴らしたとき、デスクトップ版と同じ表示になるか
- `?debug=1` で更新頻度が `sampleRate / 1024`（約 43〜47回/秒）付近、解析時間が 2ms 以下か
- 基準ピッチ変更・ホールド・ラップが動くか、バックグラウンドから戻ったときに再開できるか

---

## 8. デプロイ（GitHub Actions → GitHub Pages）

`.github/workflows/deploy.yml` の骨子（各 action のメジャーバージョンは実装時点の最新を確認すること）:

```yaml
name: Deploy to GitHub Pages
on:
  push:
    branches: [main]
  workflow_dispatch:
permissions:
  contents: read
  pages: write
  id-token: write
concurrency:
  group: pages
  cancel-in-progress: true
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: taiki-e/install-action@v2
        with:
          tool: wasm-pack
      - run: cargo test --manifest-path core/Cargo.toml
      - run: wasm-pack test --node core
      - run: node --test tests/
      - run: wasm-pack build core --release --target web --out-dir ../web/pkg
      - run: rm -f web/pkg/.gitignore
      - uses: actions/upload-pages-artifact@v3
        with:
          path: web
  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

- edition 2024 には Rust 1.85 以上が必要（stable なら問題ない）
- ユーザーに手動でやってもらうこと: **GitHub のリポジトリ設定で Settings → Pages → Source を「GitHub Actions」に設定**（リポジトリ自体は作成済み）。push はユーザーの確認を取ってから行う
- `web/pkg/` は `.gitignore` に入れ、CI でだけ生成する
- ローカル確認: `wasm-pack build core --dev --target web --out-dir ../web/pkg` → `python3 -m http.server -d web 8000` → `http://localhost:8000/`（スマホ実機での確認は Pages にデプロイしてから）

---

## 9. ステップ一覧

移植元と同じく「動くものが1つ増える」単位で区切り、各ステップでユーザーに動作確認してもらってから次へ進む。ステップごとにコミットする。

### Step 1: リポジトリの骨組みとコアのコピー
- 4章の構成を作る（`git init -b main` と `origin` の設定が済んでいなければ行う）。`core/` に移植元の3ファイルを無改変でコピーし、最小の `lib.rs`（`mod` 宣言のみ）でネイティブの `cargo test` が全部通ることを確認
- このファイルを元に新リポジトリ用の `CLAUDE.md` を作成（目的、同一性の要件、変更禁止パラメータ、コマンド、構成を簡潔に）
- ゴール: アルゴリズムが新リポジトリでも同じテスト結果になる

### Step 2: WASM API と golden テスト
- 6.1 の `analyze` / `empty_reading_text` を実装、`format_reading` テストを移植
- 7.2 の参照値で `wasm-pack test --node core` を書いて通す
- ゴール: WASM ビルドで参照値と同じ結果が出る

### Step 3: 固定データでブラウザ表示
- `index.html` + `main.js` で WASM を読み込み、JS で作ったサイン波（440Hz、1024サンプル）を `analyze` して結果文字列を表示するだけ
- ゴール: ブラウザ上でも 7.2 と同じ表示になる（ローカルサーバーで確認）

### Step 4: マイク入力をつなぐ
- 6.2 の getUserMedia + AudioWorklet + 「開始」ボタン。基音・最大ピークのテキスト表示と `?debug=1` の計測表示
- ゴール: PC ブラウザで音を鳴らすと表示が反応し、更新頻度が `sampleRate/1024` 付近になる

### Step 5: UI をデスクトップ版と同じ機能にする
- 基準ピッチ・ホールド時間のスライダー、PeakHold（＋テスト）、ラップ、スペクトルグラフ、スマホ向けレイアウト
- ゴール: デスクトップ版の全機能がそろう

### Step 6: GitHub Pages へデプロイしてスマホで確認
- 8章の workflow を追加、ユーザーに Pages の設定を依頼
- 7.3 の実機確認、iOS のバックグラウンド復帰処理
- ゴール: スマホから URL を開いて使える

### Step 7（任意）: スマホでの使い勝手
- Screen Wake Lock、PWA 化（manifest + アイコン、ホーム画面に追加）、基準ピッチ・ホールド時間を localStorage に保存（デスクトップ版にはない機能なので、入れる前にユーザーに確認する）

---

## 10. やってはいけないこと

- アルゴリズム（fft.rs / pitch.rs / pitch_detect.rs）を JS/TS で書き直す、またはコピーしたファイルを編集する
- 5章のパラメータを変える（チャンク重ね合わせで更新頻度を上げる、なども含む）
- ブラウザの音声処理（echoCancellation / noiseSuppression / autoGainControl）を有効のままにする
- `AudioContext` の `sampleRate` を強制する（リサンプリングが入る。Firefox ではマイクとレートが違うとエラーにもなる）
- 非推奨の `ScriptProcessorNode` を使う（メインスレッドで動き、取りこぼしの原因になる）
- 無音ゲート、平滑化、オクターブ補正など移植元にない処理を勝手に足す（必要ならユーザーに提案し、デスクトップ版にも入れるかを相談する）
- 依存を先回りして追加する（移植元の方針と同じく、必要になったステップで追加する）
- `opt-level = "s"`/`"z"` など、速度を犠牲にするサイズ最適化
