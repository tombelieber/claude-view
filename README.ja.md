<div align="center">

# claude-view

**Claude Code のミッションコントロール**

AIエージェントが10個動いている。1つは12分前に終了した。もう1つはコンテキスト上限に達した。3つ目はツールの承認待ち。ターミナルを <kbd>Cmd</kbd>+<kbd>Tab</kbd> で切り替えながら、月$200を盲目的に使っている。

<p>
  <a href="https://www.npmjs.com/package/claude-view"><img src="https://img.shields.io/npm/v/claude-view.svg" alt="npm version"></a>
  <a href="https://claudeview.ai"><img src="https://img.shields.io/badge/docs-claudeview.ai-orange" alt="Website"></a>
  <a href="https://www.npmjs.com/package/@claude-view/plugin"><img src="https://img.shields.io/npm/v/@claude-view/plugin.svg?label=plugin" alt="plugin version"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://discord.gg/G7wdZTpRfu"><img src="https://img.shields.io/discord/1325420051266592859?color=5865F2&logo=discord&logoColor=white&label=Discord" alt="Discord"></a>
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

<p>
  <a href="./README.md">English</a> ·
  <a href="./README.zh-TW.md">繁體中文</a> ·
  <a href="./README.zh-CN.md">简体中文</a> ·
  <a href="./README.ja.md">日本語</a> ·
  <a href="./README.es.md">Español</a> ·
  <a href="./README.fr.md">Français</a> ·
  <a href="./README.de.md">Deutsch</a> ·
  <a href="./README.pt.md">Português</a> ·
  <a href="./README.it.md">Italiano</a> ·
  <a href="./README.ko.md">한국어</a> ·
  <a href="./README.nl.md">Nederlands</a>
</p>

```bash
curl -fsSL https://get.claudeview.ai/install.sh | sh
```

**コマンド1つ。全セッション可視化。リアルタイム。**

</div>

---

## claude-view とは？

claude-view は、マシン上のすべての Claude Code セッションを監視するオープンソースダッシュボードです。ライブエージェント、過去の会話、コスト、サブエージェント、フック、ツールコールをすべて一箇所で確認できます。Rust バックエンド、React フロントエンド、約10 MBのバイナリ。設定不要、アカウント不要、100%ローカル。

**30リリース。86 MCPツール。9スキル。`npx claude-view` 1つで。**

---

## ライブモニター

実行中のすべてのセッションを一目で確認。もうターミナルタブの切り替えは不要です。

| 機能 | 説明 |
|---------|-------------|
| **セッションカード** | 各カードに最新メッセージ、モデル、コスト、ステータスを表示 — すべてのエージェントが何をしているか即座に把握 |
| **マルチセッションチャット** | VS Code スタイルのタブ（dockview）でセッションを並べて表示。ドラッグで水平・垂直分割 |
| **コンテキストゲージ** | セッションごとのコンテキストウィンドウ使用率をリアルタイム表示 — 上限に達する前に危険なエージェントを把握 |
| **キャッシュカウントダウン** | プロンプトキャッシュの有効期限を正確に把握し、トークン節約のためにメッセージのタイミングを最適化 |
| **コスト追跡** | セッション単位と合計のコスト・トークン内訳 — ホバーでモデル別の入力/出力/キャッシュ分割を表示 |
| **サブエージェントツリー** | 生成されたエージェントの完全なツリー、ステータス、コスト、使用中のツールを表示 |
| **通知サウンド** | セッション完了、エラー発生、入力待ち時にサウンド通知 — ターミナルの定期確認が不要に |
| **複数ビュー** | グリッド、リスト、カンバン、モニターモード — ワークフローに合わせて選択 |
| **カンバンスイムレーン** | プロジェクトまたはブランチ別にセッションをグループ化 — マルチプロジェクトワークフロー向けのビジュアルスイムレーンレイアウト |
| **最近クローズしたセッション** | 終了したセッションは消えずに「Recently Closed」に表示 — サーバー再起動後も保持 |
| **キュー待ちメッセージ** | キューで待機中のメッセージを「Queued」バッジ付きの保留バブルとして表示 |
| **SSE駆動** | すべてのライブデータをServer-Sent Eventsでプッシュ — キャッシュの古さリスクを完全に排除 |

---

## チャット＆会話

ライブまたは過去のセッションを閲覧、検索、操作できます。

| 機能 | 説明 |
|---------|-------------|
| **統合ライブチャット** | 履歴とリアルタイムメッセージを1つのスクロール可能な会話に表示 — タブ切り替え不要 |
| **開発者モード** | セッションごとにチャットビューと開発者ビューを切り替え。開発者モードではツールカード、イベントカード、フックメタデータ、フィルターチップ付きの完全な実行トレースを表示 |
| **完全な会話ブラウザ** | すべてのセッション、すべてのメッセージをマークダウンとコードブロックで完全レンダリング |
| **ツールコールの可視化** | ファイル読み込み、編集、bashコマンド、MCPコール、スキル呼び出しを表示 — テキストだけではない |
| **コンパクト/詳細切り替え** | 会話の概要確認と各ツールコールの詳細確認を切り替え |
| **スレッドビュー** | サブエージェント階層とインデント付きスレッドでエージェントの会話を追跡 |
| **フックイベントのインライン表示** | Pre/Postツールフックを会話ブロックとしてレンダリング — 会話の横でフックの発火を確認 |
| **エクスポート** | コンテキスト再開や共有のためのマークダウンエクスポート |
| **一括選択＆アーカイブ** | 複数セッションを選択してバッチアーカイブ、永続的なフィルター状態付き |
| **暗号化共有** | E2E暗号化リンクでセッションを共有 — AES-256-GCM、サーバーへの信頼ゼロ、鍵はURLフラグメントにのみ存在 |

---

## エージェント内部

Claude Code は「thinking...」の裏で多くの処理を行っていますが、ターミナルには表示されません。claude-view はそのすべてを公開します。

| 機能 | 説明 |
|---------|-------------|
| **サブエージェント会話** | 生成されたエージェントの完全なツリー、プロンプト、出力、エージェント別のコスト/トークン内訳 |
| **MCPサーバーコール** | 呼び出されているMCPツールとその結果 |
| **スキル/フック/プラグイン追跡** | 発火したスキル、実行されたフック、アクティブなプラグインを把握 |
| **フックイベント記録** | デュアルチャネルフックキャプチャ（ライブWebSocket + JONLバックフィル） — 過去のセッションも含めすべてのイベントを記録・閲覧可能 |
| **セッションソースバッジ** | 各セッションの起動方法を表示：Terminal、VS Code、Agent SDK、その他のエントリーポイント |
| **ワークツリーブランチ乖離** | git ワークツリーブランチの乖離を検出 — ライブモニターと履歴に表示 |
| **@File メンションチップ** | `@filename` 参照を抽出してチップとして表示 — ホバーでフルパスを確認 |
| **ツール使用タイムライン** | すべての tool_use/tool_result ペアのタイミング付きアクションログ |
| **エラー表面化** | エラーをセッションカードに浮上 — 埋もれた失敗はなし |
| **生メッセージインスペクター** | 完全な情報が必要な時、任意のメッセージの生JSONを詳細確認 |

---

## 検索

| 機能 | 説明 |
|---------|-------------|
| **全文検索** | すべてのセッションを横断して検索 — メッセージ、ツールコール、ファイルパス。Tantivy（Rustネイティブ、Luceneクラス）搭載 |
| **統合検索エンジン** | Tantivy 全文検索 + SQLite プリフィルターが並列実行 — 1エンドポイント、50ms以下の結果 |
| **プロジェクト＆ブランチフィルター** | 現在作業中のプロジェクトやブランチに絞り込み |
| **コマンドパレット** | <kbd>Cmd</kbd>+<kbd>K</kbd> でセッション間ジャンプ、ビュー切り替え、なんでも検索 |

---

## アナリティクス

Claude Code 使用状況の完全な分析スイート。Cursorのダッシュボードのようなものですが、より深い分析が可能です。

<details>
<summary><strong>ダッシュボード</strong></summary>
<br>

| 機能 | 説明 |
|---------|-------------|
| **週次比較メトリクス** | セッション数、トークン使用量、コスト — 前期間との比較 |
| **アクティビティヒートマップ** | 90日間のGitHubスタイルグリッドで日別の使用強度を表示 |
| **トップスキル/コマンド/MCPツール/エージェント** | 最もよく使用する呼び出し可能項目のリーダーボード — クリックで該当セッションを検索 |
| **最もアクティブなプロジェクト** | セッション数でランク付けされたプロジェクトの棒グラフ |
| **ツール使用内訳** | すべてのセッションにわたる編集、読み込み、bashコマンドの合計 |
| **最長セッション** | 所要時間付きでマラソンセッションにすばやくアクセス |

</details>

<details>
<summary><strong>AI貢献</strong></summary>
<br>

| 機能 | 説明 |
|---------|-------------|
| **コード出力追跡** | 追加/削除行数、変更ファイル数、コミット数 — すべてのセッションにわたって |
| **コストROI指標** | コミットあたりのコスト、セッションあたりのコスト、AI出力1行あたりのコスト — トレンドチャート付き |
| **モデル比較** | モデル別（Opus、Sonnet、Haiku）の出力と効率の横並び比較 |
| **学習曲線** | 再編集率の時系列推移 — プロンプトスキルの向上を可視化 |
| **ブランチ内訳** | セッションドリルダウン付きの折りたたみ可能なブランチ別ビュー |
| **スキル効果** | どのスキルが実際に出力を改善し、どのスキルが改善しないかを把握 |

</details>

<details>
<summary><strong>インサイト</strong> <em>（実験的）</em></summary>
<br>

| 機能 | 説明 |
|---------|-------------|
| **パターン検出** | セッション履歴から発見された行動パターン |
| **過去と現在のベンチマーク** | 初月と直近の使用状況を比較 |
| **カテゴリ内訳** | Claude の用途をツリーマップで表示 — リファクタリング、機能追加、デバッグなど |
| **AI Fluency Score** | 総合的な効果を追跡する0-100の単一スコア |

> インサイトとFluency Scoreは実験的機能です。方向性の参考としてご利用ください。確定的なものではありません。

</details>

---

## プラン、プロンプト＆チーム

| 機能 | 説明 |
|---------|-------------|
| **プランブラウザ** | `.claude/plans/` をセッション詳細で直接表示 — ファイルを探し回る必要なし |
| **プロンプト履歴** | 送信したすべてのプロンプトの全文検索、テンプレートクラスタリングとインテント分類付き |
| **チームダッシュボード** | チームリード、受信トレイメッセージ、チームタスク、全チームメンバーのファイル変更を確認 |
| **プロンプト分析** | プロンプトテンプレート、インテント分布、使用統計のリーダーボード |

---

## システムモニター

| 機能 | 説明 |
|---------|-------------|
| **ライブCPU/RAM/ディスクゲージ** | スムーズなアニメーション遷移付きでSSE経由のリアルタイムシステムメトリクスをストリーミング |
| **コンポーネントダッシュボード** | サイドカーとオンデバイスAIのメトリクスを表示：VRAM使用量、CPU、RAM、コンポーネントごとのセッション数 |
| **プロセスリスト** | 名前でグループ化、CPU順にソートされたプロセス — エージェント実行中にマシンが実際に何をしているかを把握 |

---

## オンデバイスAI

セッションフェーズ分類のためにローカルLLMを実行 — APIコールなし、追加コストなし。

| 機能 | 説明 |
|---------|-------------|
| **プロバイダー非依存** | 任意のOpenAI互換エンドポイントに接続 — oMLX、Ollama、LM Studio、または独自サーバー |
| **モデルセレクター** | RAM要件表示付きのキュレーション済みモデルレジストリから選択 |
| **フェーズ分類** | セッションに現在のフェーズ（コーディング、デバッグ、プランニングなど）をタグ付け、信頼度ゲート付き表示 |
| **スマートリソース管理** | EMA安定化分類と指数バックオフ — ナイーブポーリング比でGPU無駄を93%削減 |

---

## プラグイン

`@claude-view/plugin` は、Claude にダッシュボードデータへのネイティブアクセスを提供します — 86 MCPツール、9スキル、自動起動。

```bash
claude plugin add @claude-view/plugin
```

### 自動起動

すべての Claude Code セッションがダッシュボードを自動的に起動します。手動で `npx claude-view` を実行する必要はありません。

### 86 MCPツール

Claude向けに最適化された8つの手作りツール：

| ツール | 説明 |
|------|-------------|
| `list_sessions` | フィルター付きでセッションを閲覧 |
| `get_session` | メッセージとメトリクス付きの完全なセッション詳細 |
| `search_sessions` | すべての会話を横断する全文検索 |
| `get_stats` | ダッシュボード概要 — 総セッション数、コスト、トレンド |
| `get_fluency_score` | AI Fluency Score（0-100）と内訳 |
| `get_token_stats` | キャッシュヒット率付きトークン使用量 |
| `list_live_sessions` | 現在実行中のエージェント（リアルタイム） |
| `get_live_summary` | 本日の合計コストとステータス |

さらに、27カテゴリにわたるOpenAPIスペックから**78の自動生成ツール**（コントリビューション、インサイト、コーチング、エクスポート、ワークフローなど）。

### 9スキル

| スキル | 説明 |
|-------|-------------|
| `/session-recap` | 特定セッションの要約 — コミット、メトリクス、所要時間 |
| `/daily-cost` | 本日のコスト、実行中のセッション、トークン使用量 |
| `/standup` | スタンドアップ更新のためのマルチセッション作業ログ |
| `/coaching` | AIコーチングのヒントとカスタムルール管理 |
| `/insights` | 行動パターン分析 |
| `/project-overview` | セッション横断のプロジェクト概要 |
| `/search` | 自然言語検索 |
| `/export-data` | セッションをCSV/JSONにエクスポート |
| `/team-status` | チームアクティビティの概要 |

---

## ワークフロー

| 機能 | 説明 |
|---------|-------------|
| **ワークフロービルダー** | VS Codeスタイルのレイアウト、Mermaidダイアグラムプレビュー、YAMLエディター付きでマルチステージワークフローを作成 |
| **ストリーミングLLMチャットレール** | 組み込みチャットを通じてワークフロー定義をリアルタイムで生成 |
| **ステージランナー** | ワークフロー実行中にステージカラム、試行カード、プログレスバーを可視化 |
| **組み込みシードワークフロー** | Plan PolisherとPlan Executorが初期搭載 |

---

## IDEで開く

| 機能 | 説明 |
|---------|-------------|
| **ワンクリックでファイルを開く** | セッションで参照されたファイルをエディターで直接開く |
| **エディター自動検出** | VS Code、Cursor、Zedなどを自動検出 — 設定不要 |
| **必要な場所すべてに** | 変更タブ、ファイルヘッダー、カンバンプロジェクトヘッダーにボタンを表示 |
| **設定の記憶** | お気に入りのエディターをセッション間で記憶 |

---

## 技術構成

| | |
|---|---|
| **高速** | SIMD高速化JONLパース、メモリマップドI/Oを備えたRustバックエンド — 数千セッションを数秒でインデックス |
| **リアルタイム** | ファイルウォッチャー + SSE + ハートビート、イベントリプレイ、クラッシュリカバリ付きの多重化WebSocket |
| **軽量** | ダウンロード約10 MB、ディスク上約27 MB。ランタイム依存関係なし、バックグラウンドデーモンなし |
| **100%ローカル** | すべてのデータはマシン上に保持。デフォルトでテレメトリゼロ、必須アカウントゼロ |
| **設定不要** | `npx claude-view` で完了。APIキー不要、セットアップ不要、アカウント不要 |
| **FSM駆動** | チャットセッションは明示的なフェーズと型付きイベントを持つ有限状態機械で実行 — 決定論的、競合状態なし |

<details>
<summary><strong>数値で見る</strong></summary>
<br>

26プロジェクト、1,493セッションのM系列Macでの計測結果：

| 指標 | claude-view | 一般的なElectronダッシュボード |
|--------|:-----------:|:--------------------------:|
| **ダウンロード** | **約10 MB** | 150-300 MB |
| **ディスク上** | **約27 MB** | 300-500 MB |
| **起動** | **< 500 ms** | 3-8 s |
| **RAM（フルインデックス）** | **約50 MB** | 300-800 MB |
| **1,500セッションのインデックス** | **< 1 s** | N/A |
| **ランタイム依存関係** | **0** | Node.js + Chromium |

主要技術：SIMDプリフィルター（`memchr`）、メモリマップドJSONLパース、Tantivy全文検索、mmapからパース、レスポンスまでのゼロコピースライス。

</details>

---

## 比較

| ツール | カテゴリ | スタック | サイズ | ライブモニター | マルチセッションチャット | 検索 | アナリティクス | MCPツール |
|------|----------|-------|:----:|:------------:|:------------------:|:------:|:---------:|:---------:|
| **[claude-view](https://github.com/tombelieber/claude-view)** | モニター + ワークスペース | Rust | **約10 MB** | **対応** | **対応** | **対応** | **対応** | **86** |
| [opcode](https://github.com/winfunc/opcode) | GUI + セッション管理 | Tauri 2 | 約13 MB | 部分的 | 非対応 | 非対応 | 対応 | 非対応 |
| [ccusage](https://github.com/ryoppippi/ccusage) | CLI使用量トラッカー | TypeScript | 約600 KB | 非対応 | 非対応 | 非対応 | CLI | 非対応 |
| [CodePilot](https://github.com/op7418/CodePilot) | デスクトップチャットUI | Electron | 約140 MB | 非対応 | 非対応 | 非対応 | 非対応 | 非対応 |
| [claude-run](https://github.com/kamranahmedse/claude-run) | 履歴ビューアー | TypeScript | 約500 KB | 部分的 | 非対応 | 基本的 | 非対応 | 非対応 |

> チャットUI（CodePilot、CUI、claude-code-webui）はClaude Code *用の* インターフェースです。claude-view は既存のターミナルセッションを監視するダッシュボードです。これらは補完関係にあります。

---

## インストール

| 方法 | コマンド |
|--------|---------|
| **Shell**（推奨） | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |
| **Plugin**（自動起動） | `claude plugin add @claude-view/plugin` |

Shellインストーラーはビルド済みバイナリ（約10 MB）をダウンロードし、`~/.claude-view/bin` にインストールしてPATHに追加します。あとは `claude-view` を実行するだけです。

**唯一の要件：** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) がインストール済みであること。

<details>
<summary><strong>設定</strong></summary>
<br>

| 環境変数 | デフォルト | 説明 |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` or `PORT` | `47892` | デフォルトポートの変更 |

</details>

<details>
<summary><strong>セルフホスティング＆ローカル開発</strong></summary>
<br>

ビルド済みバイナリには認証、共有、モバイルリレーが組み込まれています。ソースからビルドする場合、これらの機能は**環境変数によるオプトイン**です — 省略するとその機能は無効になります。

| 環境変数 | 機能 | 未設定時 |
|-------------|---------|------------|
| `SUPABASE_URL` | ログイン / 認証 | 認証無効 — 完全ローカル、アカウント不要モード |
| `RELAY_URL` | モバイルペアリング | QRペアリング利用不可 |
| `SHARE_WORKER_URL` + `SHARE_VIEWER_URL` | 暗号化共有 | 共有ボタン非表示 |

```bash
bun dev    # 完全ローカル、クラウド依存関係なし
```

</details>

<details>
<summary><strong>エンタープライズ / サンドボックス環境</strong></summary>
<br>

マシンの書き込みが制限されている場合（DataCloak、CrowdStrike、企業DLP）：

```bash
cp crates/server/.env.example .env
# CLAUDE_VIEW_DATA_DIR のコメントを解除
```

これによりデータベース、検索インデックス、ロックファイルがリポジトリ内に保持されます。読み取り専用環境ではフック登録をスキップするために `CLAUDE_VIEW_SKIP_HOOKS=1` を設定してください。

</details>

---

## FAQ

<details>
<summary><strong>ログインしているのに「Not signed in」バナーが表示される</strong></summary>
<br>

claude-view は `~/.claude/.credentials.json`（macOS キーチェーンフォールバック付き）を読み取ってClaude認証情報を確認します。以下の手順を試してください：

1. **Claude CLI認証を確認：** `claude auth status`
2. **認証情報ファイルを確認：** `cat ~/.claude/.credentials.json` — `accessToken` を持つ `claudeAiOauth` セクションがあるはずです
3. **macOSキーチェーンを確認：** `security find-generic-password -s "Claude Code-credentials" -w`
4. **トークンの有効期限を確認：** 認証情報JSONの `expiresAt` を確認 — 期限切れの場合は `claude auth login` を実行
5. **HOMEを確認：** `echo $HOME` — サーバーは `$HOME/.claude/.credentials.json` から読み取ります

すべてのチェックに合格してもバナーが表示される場合は、[Discord](https://discord.gg/G7wdZTpRfu) でご報告ください。

</details>

<details>
<summary><strong>claude-view はどのデータにアクセスしますか？</strong></summary>
<br>

claude-view は Claude Code が `~/.claude/projects/` に書き込むJSONLセッションファイルを読み取ります。SQLite と Tantivy を使用してローカルにインデックスを作成します。暗号化共有機能を明示的に使用しない限り、**データはマシンから外に出ません**。テレメトリはオプトインで、デフォルトではオフです。

</details>

<details>
<summary><strong>VS Code / Cursor / IDE拡張機能の Claude Code で動作しますか？</strong></summary>
<br>

はい。claude-view は起動方法に関係なく、すべての Claude Code セッションを監視します — ターミナルCLI、VS Code拡張機能、Cursor、Agent SDK。各セッションにはソースバッジ（Terminal、VS Code、SDK）が表示されるため、起動方法でフィルタリングできます。

</details>

---

## コミュニティ

- **ウェブサイト：** [claudeview.ai](https://claudeview.ai) — ドキュメント、変更履歴、ブログ
- **Discord：** [サーバーに参加](https://discord.gg/G7wdZTpRfu) — サポート、機能リクエスト、ディスカッション
- **プラグイン：** [`@claude-view/plugin`](https://www.npmjs.com/package/@claude-view/plugin) — 86 MCPツール、9スキル、自動起動

---

<details>
<summary><strong>開発</strong></summary>
<br>

前提条件：[Rust](https://rustup.rs/)、[Bun](https://bun.sh/)、`cargo install cargo-watch`

```bash
bun install        # すべてのワークスペース依存関係をインストール
bun dev            # フルスタック開発を開始（Rust + Web + Sidecar、ホットリロード付き）
```

### ワークスペースレイアウト

| パス | パッケージ | 用途 |
|------|---------|---------|
| `apps/web/` | `@claude-view/web` | React SPA（Vite） — メインWebフロントエンド |
| `apps/share/` | `@claude-view/share` | 共有ビューアーSPA — Cloudflare Pages |
| `apps/mobile/` | `@claude-view/mobile` | Expo ネイティブアプリ |
| `apps/landing/` | `@claude-view/landing` | Astro 5 ランディングページ（クライアントサイドJSゼロ） |
| `packages/shared/` | `@claude-view/shared` | 共有型 & テーマトークン |
| `packages/design-tokens/` | `@claude-view/design-tokens` | カラー、スペーシング、タイポグラフィ |
| `packages/plugin/` | `@claude-view/plugin` | Claude Code プラグイン（MCPサーバー + ツール + スキル） |
| `crates/` | — | Rust バックエンド（Axum） |
| `sidecar/` | — | Node.js サイドカー（Agent SDK ブリッジ） |
| `infra/share-worker/` | — | Cloudflare Worker — 共有API（R2 + D1） |
| `infra/install-worker/` | — | Cloudflare Worker — ダウンロード追跡付きインストールスクリプト |

### 開発コマンド

| コマンド | 説明 |
|---------|-------------|
| `bun dev` | フルスタック開発 — Rust + Web + Sidecar、ホットリロード付き |
| `bun run dev:web` | Webフロントエンドのみ |
| `bun run dev:server` | Rustバックエンドのみ |
| `bun run build` | すべてのワークスペースをビルド |
| `bun run preview` | Webをビルドしてリリースバイナリで配信 |
| `bun run lint:all` | JS/TS + Rust（Clippy）のリント |
| `bun run typecheck` | TypeScript の型チェック |
| `bun run test` | すべてのテストを実行（Turbo） |
| `bun run test:rust` | Rust テストを実行 |
| `bun run storybook` | コンポーネント開発用にStorybookを起動 |
| `bun run dist:test` | ビルド + パック + インストール + 実行（完全なdistテスト） |

### リリース

```bash
bun run release          # パッチバンプ
bun run release:minor    # マイナーバンプ
git push origin main --tags    # CIをトリガー → ビルド → npmに自動公開
```

</details>

---

## プラットフォームサポート

| プラットフォーム | ステータス |
|----------|--------|
| macOS (Apple Silicon) | 対応済み |
| macOS (Intel) | 対応済み |
| Linux (x64) | 予定 |
| Windows (x64) | 予定 |

---

## 関連

- **[claudeview.ai](https://claudeview.ai)** — 公式ウェブサイト、ドキュメント、変更履歴
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** — 86 MCPツールと9スキルを備えた Claude Code プラグイン。`claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** — Claude Code は30日後にセッションを削除します。これがセッションを保存します。`npx claude-backup`

---

<div align="center">

**claude-view** がAIエージェントの動きを可視化するのに役立ったら、スターをご検討ください。

<a href="https://github.com/tombelieber/claude-view/stargazers">
  <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
</a>

<br><br>

MIT &copy; 2026

</div>
