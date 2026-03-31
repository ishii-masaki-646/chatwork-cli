# chatwork-cli

`clap` ベースの Chatwork 向け定型文送信 CLI です。

## できること

- テンプレート一覧の表示
- テンプレート本文のプレビュー
- 変数を差し込んだうえでの Chatwork 送信
- 自分のアカウント情報の取得

## 前提

- Rust / Cargo
- Chatwork API トークン

`CHATWORK_API_TOKEN` は次の優先順位で解決します。

1. 通常の環境変数
2. カレントディレクトリから親ディレクトリへ向かって探索した `.env`
3. `~/.config/chatwork-cli/.env`

```bash
export CHATWORK_API_TOKEN=your_token
```

`.env` を利用する場合は、次の形式で設定してください。

```dotenv
CHATWORK_API_TOKEN=your_token
```

初期設定時は `.env.example` をコピーして `.env` を作成してください。`~/.config/chatwork-cli/.env` に配置して共通設定として使うこともできます。

```bash
cp .env.example .env
```

`./scripts/build` を実行すると、利用しやすい形で `bin/chatwork` にバイナリを配置できます。

## 補完

`chatwork completion <shell>` で補完スクリプトを標準出力へ出力できます。`completion` サブコマンドは設定ファイルがなくても利用できます。

`send` および `template show` の位置では、設定ファイルから読み取ったテンプレート名も補完候補に表示されます。

```bash
./bin/chatwork completion bash > ~/.local/share/bash-completion/completions/chatwork
mkdir -p ~/.zfunc
./bin/chatwork completion zsh > ~/.zfunc/_chatwork
```

`zsh` で利用する場合は、必要に応じて `~/.zshrc` に `fpath=(~/.zfunc $fpath)` および `autoload -Uz compinit && compinit` を追加してください。

## i18n

CLI の実行時メッセージは gettext 風の `msgid` ベースで管理しています。翻訳カタログは `locale/<lang>/LC_MESSAGES/chatwork-cli.po` に配置します。既定では日本語カタログを内蔵しており、`CHATWORK_LOCALE` や `LANG` でロケールを切り替えられます。

外部カタログを利用する場合は、`CHATWORK_LOCALE_DIR` で `locale/` の配置先を上書きしてください。

## 設定ファイル

既定の設定ファイルパスは `~/.config/chatwork-cli/config.toml` です。サンプルは [config/config.example.toml](/home/ishii/work/myrepo/chatwork-cli/config/config.example.toml) にあります。

テンプレート本文は `body` に直接記述するか、`body_file` で外部ファイルを指定できます。`templates_prefix` を省略した場合の既定値は `~/.config/chatwork-cli/templates` です。`body_file` の相対パスはこのディレクトリを基準に解決されます。

```toml
default_room_id = "123456789"
templates_prefix = "~/.config/chatwork-cli/templates"

[templates.follow_up]
description = "確認依頼のフォロー"
body_file = "follow_up.txt"

[templates.reminder]
room_id = "987654321"
body = """
[info][title]リマインド[/title]
{{message}}
[/info]
"""
```

## 使い方

```bash
cargo run -- get me
cargo run -- get me --format=plain
cargo run -- template list --config ./config/config.example.toml
cargo run -- template show follow_up --config ./config/config.example.toml --var to_id=12345 --var topic=見積
cargo run -- send follow_up --config ./config/config.example.toml --room 123456 --var to_id=12345 --var topic=見積 --dry-run
```

`get me` は既定で整形済み JSON を出力します。`--format=json-minify` で 1 行 JSON、`--format=plain` で `key=value` 形式に切り替えられます。

`bin/` に出力したバイナリを利用する場合は次のとおりです。

```bash
./scripts/build
./bin/chatwork template list --config ./config/config.example.toml
```

リリースビルドを配置する場合は次のとおりです。

```bash
./scripts/build --release
./bin/chatwork template list --config ./config/config.example.toml
```

実際に送信する場合は `--dry-run` を外してください。

`send` の送信先ルームは、次の優先順位で決定されます。

1. `--room`
2. テンプレートの `room_id`
3. `default_room_id`
