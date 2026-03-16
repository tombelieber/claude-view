# claude-view

<p align="center">
  <strong>Claude Code パワーユーザーのためのライブモニター＆コパイロット。</strong>
</p>

<p align="center">
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

<p align="center">
  <a href="https://www.npmjs.com/package/claude-view"><img src="https://img.shields.io/npm/v/claude-view.svg" alt="npm version"></a>
  <a href="https://claudeview.ai"><img src="https://img.shields.io/badge/Website-claudeview.ai-orange" alt="Website"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-macOS-lightgrey.svg" alt="macOS">
  <a href="https://discord.gg/G7wdZTpRfu"><img src="https://img.shields.io/discord/1325420051266592859?color=5865F2&logo=discord&logoColor=white&label=Discord" alt="Discord"></a>
  <a href="https://github.com/tombelieber/claude-view/stargazers"><img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=social" alt="GitHub stars"></a>
</p>

---

## 問題

3つのプロジェクトを開いている。各プロジェクトには複数のgitワークツリーがある。各ワークツリーでは複数のClaude Codeセッションが実行中。考え中のもの、あなたの入力を待っているもの、コンテキスト制限に近づいているもの、そして10分前に完了したのに忘れていたもの。

Cmd-Tabで15のターミナルウィンドウを切り替え、どのセッションが何をしていたか思い出そうとしている。キャッシュが切れたのに気づかず、トークンを無駄にしている。すべてを一覧できる場所がないので、フローを失っている。そして「思考中...」のスピナーの裏で、Claudeはサブエージェントを生成し、MCPサーバーを呼び出し、スキルを実行し、フックを発火させている——そのどれも見えない。

**Claude Codeは非常に強力です。しかしダッシュボードなしで10以上の同時セッションを操るのは、速度計のない車を運転するようなものです。**

## ソリューション

**claude-view**は、Claude Codeセッションと並行して動作するリアルタイムダッシュボードです。ブラウザタブ1つで、すべてのセッションが見え、コンテキストが一目瞭然。

```bash
curl -fsSL https://get.claudeview.ai/install.sh | sh
```

それだけです。ブラウザで開きます。すべてのセッション——ライブも過去も——1つのワークスペースに。

---

## 機能一覧

### ライブモニター

| 機能 | 重要な理由 |
|---------|---------------|
| **最後のメッセージ付きセッションカード** | 長時間実行中の各セッションが何をしているか即座に把握 |
| **通知サウンド** | セッションが完了または入力が必要な時に通知——ターミナルのポーリングを止められる |
| **コンテキストゲージ** | セッションごとのリアルタイムコンテキストウィンドウ使用量——危険ゾーンのものを把握 |
| **キャッシュウォームカウントダウン** | プロンプトキャッシュの有効期限を正確に把握し、トークン節約のタイミングを計れる |
| **コスト追跡** | セッションごとと全体の支出——ホバーでトークン/コスト内訳とカテゴリ別キャッシュ節約を表示 |
| **サブエージェント可視化** | エージェントツリーの全体像——サブエージェント、そのステータス、呼び出しているツール |
| **複数ビュー** | グリッド、リスト、Kanban、モニターモード——ワークフローに合わせて選択 |
| **Kanbanスイムレーン** | プロジェクトまたはブランチでセッションをグループ化——マルチプロジェクトワークフローのビジュアルスイムレーンレイアウト |

### リッチなチャット履歴

| 機能 | 重要な理由 |
|---------|---------------|
| **完全な会話ブラウザ** | すべてのセッション、すべてのメッセージ、markdownとコードブロック完全レンダリング |
| **ツールコール可視化** | ファイル読み取り、編集、bashコマンド、MCP呼び出し、スキル実行——テキストだけでなく |
| **コンパクト/詳細切替** | 会話をざっと見るか、すべてのツールコールを掘り下げるか |
| **スレッドビュー** | サブエージェント階層でエージェントの会話を追跡 |
| **エクスポート** | コンテキスト再開や共有のためのMarkdownエクスポート |
| **一括選択＆アーカイブ** | 複数セッションを選択してバッチアーカイブ——フィルター状態を保持 |
| **暗号化共有** | E2E暗号化リンクで任意のセッションを共有——サーバー信頼ゼロ |

### 高度な検索

| 機能 | 重要な理由 |
|---------|---------------|
| **全文検索** | すべてのセッションを横断して検索——メッセージ、ツールコール、ファイルパス |
| **プロジェクト＆ブランチフィルター** | 今作業中のプロジェクトにスコープを絞る |
| **コマンドパレット** | Cmd+Kでセッション間ジャンプ、ビュー切替、何でも検索 |

### エージェント内部——隠れたものを見る

Claude Codeは「思考中...」の裏で多くのことを行っており、ターミナルには表示されません。claude-viewはそのすべてを明らかにします。

| 機能 | 重要な理由 |
|---------|---------------|
| **サブエージェント会話** | 生成されたエージェントの完全なツリー、プロンプト、出力を確認 |
| **MCPサーバー呼び出し** | どのMCPツールが呼び出され、その結果を確認 |
| **スキル/フック/プラグイン追跡** | どのスキルが発火し、どのフックが実行され、どのプラグインがアクティブかを把握 |
| **フックイベント記録** | デュアルチャネルフックキャプチャ（ライブ + JSONL バックフィル）——過去のセッションを含むすべてのフックイベントが記録されブラウズ可能 |
| **ワークツリーブランチドリフト** | gitワークツリーブランチの分岐を検出——ライブモニターと履歴に表示 |
| **ツール使用タイムライン** | すべてのtool_use/tool_resultペアとタイミングのアクションログ |
| **エラー表出** | エラーがセッションカードに浮上——埋もれた失敗はもうない |
| **生メッセージインスペクター** | 全体像が必要な時、任意のメッセージの生JSONを掘り下げ |

### アナリティクス

Claude Code使用のための豊富な分析スイート。Cursorのダッシュボードを思い浮かべてください、ただしもっと深く。

**ダッシュボード概要**

| 機能 | 説明 |
|---------|-------------|
| **週ごとの指標** | セッション数、トークン使用量、コスト——前の期間と比較 |
| **アクティビティヒートマップ** | 90日間のGitHubスタイルのグリッドで日々のClaude Code使用強度を表示 |
| **トップスキル/コマンド/MCPツール/エージェント** | 最も使用した呼び出し可能項目のリーダーボード——クリックでマッチするセッションを検索 |
| **最もアクティブなプロジェクト** | セッション数でランク付けされたプロジェクトの棒グラフ |
| **ツール使用内訳** | すべてのセッションにわたる編集、読み取り、bashコマンドの合計 |
| **最長セッション** | 持続時間付きのマラソンセッションへのクイックアクセス |

**AI貢献**

| 機能 | 説明 |
|---------|-------------|
| **コード出力追跡** | 追加/削除行数、変更ファイル、全セッションのコミット数 |
| **コストROI指標** | コミットあたりのコスト、セッションあたりのコスト、AI出力1行あたりのコスト——トレンドチャート付き |
| **モデル比較** | モデル別（Opus、Sonnet、Haiku）の出力と効率のサイドバイサイド比較 |
| **学習曲線** | 時間経過に伴う再編集率——プロンプティングの上達を確認 |
| **ブランチ内訳** | セッションドリルダウン付きの折りたたみ可能なブランチごとビュー |
| **スキル効果** | どのスキルが実際にアウトプットを改善し、どれがしないか |

**インサイト** *(実験的)*

| 機能 | 説明 |
|---------|-------------|
| **パターン検出** | セッション履歴から発見された行動パターン |
| **当時 vs 今のベンチマーク** | 最初の1ヶ月と最近の使用を比較 |
| **カテゴリ内訳** | Claudeの用途のツリーマップ——リファクタリング、機能、デバッグなど |
| **AI流暢度スコア** | 全体的な効果を追跡する0-100の単一スコア |

> **注意:** インサイトと流暢度スコアは初期実験段階です。方向性の指標としてお考えください。

---

## フローのために設計

claude-viewは以下のような開発者のために設計されています：

- **3以上のプロジェクト**を同時に実行し、各プロジェクトに複数のワークツリー
- 常時**10-20のClaude Codeセッション**を開いている
- 何が実行中か見失わずに素早くコンテキストスイッチが必要
- キャッシュウィンドウに合わせてメッセージのタイミングを計り**トークン支出を最適化**したい
- エージェントを確認するためにターミナルをCmd-Tabで切り替えることにフラストレーションを感じている
- **ワークツリー対応**——gitワークツリー間のブランチドリフトを検出

---

## 技術構成

| | |
|---|---|
| **超高速** | SIMD加速JSONLパース、メモリマップドI/OのRustバックエンド——数千セッションを数秒でインデックス |
| **リアルタイム** | ファイルウォッチャー + SSE + 統合WebSocket（ハートビート、イベントリプレイ、クラッシュリカバリ対応） |
| **小さなフットプリント** | ~10 MBダウンロード、~27 MBディスク使用。ランタイム依存なし、バックグラウンドデーモンなし |
| **100%ローカル** | すべてのデータはあなたのマシンに。テレメトリゼロ、アカウント不要。オプションの暗号化共有機能あり。 |
| **ゼロ設定** | `npx claude-view`で完了。APIキー不要、セットアップ不要、アカウント不要 |

---

## インストール

| 方法 | コマンド |
|--------|---------|
| **Shell**（推奨） | `curl -fsSL https://get.claudeview.ai/install.sh \| sh` |
| **npx** | `npx claude-view` |

Shellインストーラーはプリビルドバイナリ（~10 MB）をダウンロードし、`~/.claude-view/bin`にインストールしてPATHに追加します。その後は`claude-view`を実行するだけです。

### 設定

| 環境変数 | デフォルト | 説明 |
|-------------|---------|-------------|
| `CLAUDE_VIEW_PORT` または `PORT` | `47892` | デフォルトポートを上書き |
| `CLAUDE_VIEW_DATA_DIR` | `~/Library/Caches/claude-view` | データディレクトリを上書き |

---

**唯一の要件：**[Claude Code](https://docs.anthropic.com/en/docs/claude-code)がインストール済み——監視するセッションファイルが作成されます。

---

## 比較

他のツールはビューアー（履歴ブラウズ）か、シンプルなモニターです。リアルタイム監視、リッチなチャット履歴、デバッグツール、高度な検索を1つのワークスペースに統合したものはありません。

```
                    パッシブ ←————————————→ アクティブ
                         |                  |
            閲覧のみ    |  ccusage         |
                         |  History Viewer  |
                         |  clog            |
                         |                  |
            モニター    |  claude-code-ui  |
            のみ         |  Agent Sessions  |
                         |                  |
            完全な      |  ★ claude-view   |
            ワークスペース |                |
```

---

## コミュニティ

[Discordサーバー](https://discord.gg/G7wdZTpRfu)でサポート、機能リクエスト、ディスカッションに参加してください。

---

## このプロジェクトが気に入りましたか？

**claude-view**がClaude Codeの活用に役立ったなら、スターの付与をご検討ください。他の方がこのツールを発見する助けになります。

<p align="center">
  <a href="https://github.com/tombelieber/claude-view/stargazers">
    <img src="https://img.shields.io/github/stars/tombelieber/claude-view?style=for-the-badge&logo=github" alt="Star on GitHub">
  </a>
</p>

---

## 開発

前提条件：[Rust](https://rustup.rs/)、[Bun](https://bun.sh/)、`cargo install cargo-watch`

```bash
bun install        # フロントエンド依存関係のインストール
bun dev            # フルスタック開発を開始（Rust + Vite ホットリロード付き）
```

| コマンド | 説明 |
|---------|-------------|
| `bun dev` | フルスタック開発——Rust変更時自動再起動、Vite HMR |
| `bun dev:server` | Rustバックエンドのみ（cargo-watch付き） |
| `bun dev:client` | Viteフロントエンドのみ（バックエンド起動前提） |
| `bun run build` | 本番用フロントエンドビルド |
| `bun run preview` | ビルド + リリースバイナリで配信 |
| `bun run lint` | フロントエンド（ESLint）とバックエンド（Clippy）のリント |
| `bun run fmt` | Rustコードのフォーマット |
| `bun run check` | 型チェック + リント + テスト（コミット前ゲート） |
| `bun test` | Rustテストスイート実行（`cargo test --workspace`） |
| `bun test:client` | フロントエンドテスト実行（vitest） |
| `bun run test:e2e` | Playwrightエンドツーエンドテスト実行 |

### 本番配布のテスト

```bash
bun run dist:test    # 1コマンド：ビルド → パック → インストール → 実行
```

またはステップごとに：

| コマンド | 説明 |
|---------|-------------|
| `bun run dist:pack` | バイナリ + フロントエンドを`/tmp/`にtarballとしてパッケージ |
| `bun run dist:install` | tarballを`~/.cache/claude-view/`に展開（初回ダウンロードをシミュレート） |
| `bun run dist:run` | キャッシュされたバイナリでnpxラッパーを実行 |
| `bun run dist:test` | 上記すべてを1コマンドで |
| `bun run dist:clean` | すべてのdistキャッシュと一時ファイルを削除 |

### リリース

```bash
bun run release          # パッチバンプ：0.1.0 → 0.1.1
bun run release:minor    # マイナーバンプ：0.1.0 → 0.2.0
bun run release:major    # メジャーバンプ：0.1.0 → 1.0.0
```

`npx-cli/package.json`のバージョンをバンプし、コミットし、gitタグを作成します。その後：

```bash
git push origin main --tags    # CIをトリガー → 全プラットフォームビルド → npmに自動パブリッシュ
```

---

## プラットフォームサポート

| プラットフォーム | ステータス |
|----------|--------|
| macOS (Apple Silicon) | 利用可能 |
| macOS (Intel) | 利用可能 |
| Linux (x64) | 予定 |
| Windows (x64) | 予定 |

---

## 関連プロジェクト

- **[claudeview.ai](https://claudeview.ai)** — 公式サイト、ドキュメント、変更履歴
- **[@claude-view/plugin](https://www.npmjs.com/package/@claude-view/plugin)** — Claude Code プラグイン。8つの MCP ツールと3つのスキルを提供。`claude plugin add @claude-view/plugin`
- **[claude-backup](https://github.com/tombelieber/claude-backup)** — Claude Code は30日後にセッションを削除します。このツールで保存できます。`npx claude-backup`

---

## ライセンス

MIT © 2026
