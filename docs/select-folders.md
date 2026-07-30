# 仮想フォルダ

BMZ の選曲ルートには、beatoraja の標準フォルダに相当する仮想フォルダを表示します。
組み込み定義は `crates/bmz-player/resources/select-folders.toml` にあり、次を含みます。

- LAMP UPDATE / SCORE UPDATE（直近30日）
- MY BEST
- CLEAR TYPE / SCORE RANK
- DENSITY（AVERAGE / PEAK / END）
- FEATURE（皿率 / LN率）
- LEVEL（5K / 7K / 9K / 10K / 14K）
- NEW（当日から7日前、および直近7日）

FAVORITE は既存のコレクション機能を使います。INVISIBLE は実装しません。
同一曲フォルダは選択中の曲に対する既存の same-folder 操作を使います。

## プロファイル別定義

`data/profiles/<profile>/select-folders.toml` を置くと、組み込み定義を上書きできます。
同じトップレベル `id` は置換され、`enabled = false` なら非表示になります。
新しい `id` は組み込みフォルダの後ろに追加されます。

最小の1行条件は次の形式です。

```toml
version = 1

[[folders]]
id = "level-12"
name = "LEVEL 12"
query = "mode == '7K' && level == 12"
```

階層は `items` で定義します。

```toml
version = 1

[[folders]]
id = "practice"
name = "PRACTICE"
items = [
  { id = "unplayed", name = "UNPLAYED", query = "play_count == 0" },
  { id = "failed", name = "FAILED", query = "clear == 1" },
]
```

抽出後に順位を付けて件数を制限する場合、`query` をテーブルにします。

```toml
[[folders]]
id = "most-played"
name = "MOST PLAYED"

[folders.query]
filter = "play_count > 0"
order_by = "play_count desc"
limit = 20
```

連番フォルダは生成できます。`{value}`、`{ordinal}`（value + 1）、
`{days_ago}`（0ならTODAY、それ以外は「N DAYS AGO」）を置換します。

```toml
[[folders]]
id = "recent"
name = "RECENT"

[folders.generate]
values = "0..=6"
id = "day-{value}"
name = "{days_ago}"
query = "added_at in local_day({value})"
```

`items` と併用する場合は `insert_at = 0` のように生成行の挿入位置も指定できます。
省略時は既存 `items` の後ろへ追加します。

数値範囲を多数作る場合は `buckets` を使えます。各区間は下限を含み、上限を含みません。

```toml
[[folders]]
id = "density"
name = "DENSITY"

[folders.buckets]
field = "density"
prefix = "DENSITY"
cuts = [3, 5, 7, 9, 10]
```

## 1行クエリ

クエリはSQLではなく、選曲メタデータだけを参照できる型付きDSLです。
`&&`、`||`、`!`、括弧、`==`、`!=`、`<`、`<=`、`>`、`>=`、
`in [値, ...]` を使えます。

| フィールド | 値 |
|---|---|
| `mode` | `"5K"`、`"7K"`、`"9K"`、`"10K"`、`"14K"` など |
| `level` | BMSのプレイレベル |
| `density` | 平均密度 |
| `peak_density` | 最大密度 |
| `end_density` | 終盤密度 |
| `scratch_rate` | 全ノーツに対する皿ノーツの比率（0.0〜1.0） |
| `long_note_rate` | 全ノーツに対するLNの比率（0.0〜1.0） |
| `clear` | 0=未プレイ、1=FAILED、2/3=ASSIST、4=EASY、5=NORMAL、6=HARD、7=EX HARD、8以上=FULL COMBO以上 |
| `score_rate` | EXスコア率（0〜100） |
| `play_count` | プレイ回数 |
| `added_at` | ライブラリへの初回登録日時 |
| `lamp_updated_at` | ランプを更新したローカルプレイ日時 |
| `score_updated_at` | EXスコアを更新したローカルプレイ日時 |

日時にはOSのローカル日付を使います。

```text
added_at in local_day(0)          # 今日
lamp_updated_at in local_day(3)   # 4日前
added_at in local_days(7)         # 今日を含む直近7暦日
```

未知のフィールド、壊れた式、重複した兄弟 `id` は定義ロード時にエラーになります。
