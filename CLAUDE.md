# F0 Scope

デスクトップ版の音程検出アプリ `FFTexample`（Rust + cpal + egui、`/Users/tanakakisaku/Documents/dev/rust/FFTexample`）を、スマホのブラウザで使える GitHub Pages 版として作り直したもの。詳細な仕様・経緯は [WEB_PORT_PLAN.md](WEB_PORT_PLAN.md) を参照。ユーザーとの会話は日本語。

- 公開 URL: https://kisakutanaka.github.io/F0Scope/
- リモート: `git@github.com:kisakutanaka/F0Scope.git`（`main` への push でデプロイ。push はユーザーの確認を取ってから）

## 目的: デスクトップ版と「全く同じ精度と速度」

- 精度: 同じ入力なら同じ基音/最大ピーク周波数（±0.05 cent 以内）・音名・オクターブ。アルゴリズムは Rust コードをそのまま WASM 化して担保する
- 速度: 1回の解析は 1024 サンプル、重なりなし・抜けなしの連続チャンク。更新間隔 = 1024 / サンプルレート
- 挙動: 基音選定・ピークホールド・ラップ・基準ピッチ変更はデスクトップ版と同じ。表示文字列も Rust 側で整形する

## 構成

```
core/            Rust クレート pitch_core（wasm-bindgen で WASM 化）
  src/lib.rs     公開 API（analyze, empty_reading_text）と format_reading
  src/fft.rs, pitch.rs, pitch_detect.rs   移植元からコピー（pitch_detect.rs の dtft_magnitude のみ変更）
web/             GitHub Pages で公開する静的ファイル（バンドラなし、参照はすべて相対パス）
  pkg/           wasm-pack の出力（.gitignore 対象、CI で生成）
tests/           JS のテスト（node --test）
.github/workflows/deploy.yml
```

## 守ること

- `core/src/{fft,pitch,pitch_detect}.rs` は移植元のコピー。移植元が更新されたら再コピーで追従する。アルゴリズムを JS/TS で書き直さない
- 例外: `pitch_detect.rs` の `dtft_magnitude` は、サンプルごとの cos/sin を f64 の回転の漸化式に置き換えている（WASM では cos/sin が遅く、スマホで解析 4〜5ms かかっていたため。参照値との差は最大 0.003 cent）。再コピー時はこの変更を当て直す。ロジックの最適化は golden テスト（±0.05 cent）が通る範囲ならユーザーの了承済み
- 速度の目標: 解析1回が更新間隔（約 21ms）に十分収まること
- 変更禁止パラメータ（ユーザーの明示的な指示がない限り変えない）:
  - チャンクサイズ 1024（JS の `CHUNK_SIZE`）。チャンクの重ね合わせもしない
  - コア内の定数（`COARSE_CANDIDATES = 5`, `FUNDAMENTAL_THRESHOLD_RATIO = 0.2`, `ZOOM_SEARCH_WIDTH_IN_BINS = 1.0`, `ZOOM_SEARCH_POINTS = 21`, `COARSE_SPECTRUM_DISPLAY_MAX_HZ = 5000`, Hann 窓）
  - 基準ピッチ A4: 既定 440.0、範囲 415.0〜466.0、刻み 0.1
  - ピークホールド時間: 既定 1.0 秒、範囲 0.2〜5.0 秒、刻み 0.1
  - `opt-level = 3`（`"s"`/`"z"` にしない）
- 音声入力: `getUserMedia` は echoCancellation / noiseSuppression / autoGainControl をすべて `false`。`AudioContext` の `sampleRate` は指定しない（解析には `ctx.sampleRate` を渡す）。AudioWorklet を使い、`ScriptProcessorNode` は使わない
- lib.rs に `std::time::Instant` を持ち込まない（wasm32 で panic する）。時間管理は JS 側
- 無音ゲート・平滑化・オクターブ補正など移植元にない処理を勝手に足さない。依存も必要になったステップで追加する

## コマンド

```sh
cargo test --manifest-path core/Cargo.toml             # コアのネイティブテスト
wasm-pack test --node core                             # WASM ビルドでの golden テスト
node --test tests/                                     # JS のテスト
wasm-pack build core --dev --target web --out-dir ../web/pkg   # ローカル用ビルド
python3 -m http.server -d web 8000                     # http://localhost:8000/ で確認
```

## 進め方

WEB_PORT_PLAN.md「9. ステップ一覧」の順に、動くものが1つ増える単位で進める。各ステップでユーザーに動作確認してもらい、ステップごとにコミットする。
