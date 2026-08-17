# Everything 連携

Windows では Everything 1.5 以降のインデックスを使って、曲スキャンのファイル探索を高速化できます。既定値は OFF です。

## 有効化

アプリの設定画面で「スキャン設定」→「曲探索に Everything を使用（Windows）」を有効にします。`data/config.toml` では次の設定に対応します。

```toml
[scan]
use_everything = true
```

Everything のデータベースが未ロード、IPC が利用不可、ファイルサイズまたは更新日時が未インデックス、検索が失敗・タイムアウトした場合は、そのルートだけ通常のファイルシステム探索へ自動的にフォールバックします。「シンボリックリンクを辿る」が OFF の場合も、通常探索との意味を変えないためフォールバックします。

Everything 経由でも通常探索と同じく、BMS系拡張子、更新日時、ファイルサイズ、同一フォルダの `.txt` 有無、再帰設定、ドットで始まるファイル / フォルダの除外を扱います。

## ON/OFF 計測

設定を保存せず、一回の CLI スキャンだけ探索方法を上書きできます。

```powershell
cargo run -p bmz-player -- songs load --everything
cargo run -p bmz-player -- songs load --no-everything
```

出力の `Timing` で全体時間と探索時間、`Discovery backends` で実際に使われたルート数とフォールバック数を確認します。

## 2026-08-17 実測

- Everything 1.5.0.1418b
- Windows、8ルート、72,425譜面
- 既存DBに対する増分スキャン
- ON/OFF 各5回の中央値

| 指標 | OFF（通常探索） | ON（Everything） | 改善 |
| --- | ---: | ---: | ---: |
| 探索 | 5,920 ms | 476 ms | 12.4倍（92.0%短縮） |
| スキャン全体 | 6,933 ms | 1,376 ms | 5.0倍（80.2%短縮） |

両方式とも検出数は 72,425 件で一致しました。測定時の26件の parse failure は未対応の24K / 48K BMSONで、探索方式による差ではありません。
