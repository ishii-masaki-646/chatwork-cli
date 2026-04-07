# chatwork-cli

`clap` ベースの Chatwork 向け定型文送信 CLI です。

- English: [README.md](../../README.md)
- 日本語: [docs/ja/README.md](./README.md)

## できること

- テンプレート一覧の表示
- テンプレート本文のプレビュー
- 変数を差し込んだうえでの Chatwork 送信
- 自分のアカウント情報の取得
- Chatwork ファイルのダウンロード

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
# CHATWORK_DEFAULT_DOWNLOAD_DIR=~/Downloads
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

既定の設定ファイルパスは `~/.config/chatwork-cli/config.toml` です。サンプルは [config/config.example.toml](../../config/config.example.toml) にあります。

テンプレート本文は `body` に直接記述するか、`body_file` で外部ファイルを指定できます。`templates_prefix` を省略した場合の既定値は `~/.config/chatwork-cli/templates` です。`body_file` の相対パスはこのディレクトリを基準に解決されます。

```toml
default_room_id = "12345678"
templates_prefix = "~/.config/chatwork-cli/templates"

[templates.follow_up]
description = "確認依頼のフォロー"
body_file = "follow_up.txt"

[templates.reminder]
room_id = "23456789"
body = """
[info][title]リマインド[/title]
{{message}}
[/info]
"""
```

## 使い方

### 情報取得

```bash
cargo run -- get me
cargo run -- get me --format=plain
cargo run -- get status
cargo run -- get my-status --format=plain
cargo run -- get contacts --format=json-minify
cargo run -- get contacts --aids=1234567,7654321 --format=json-minify
cargo run -- get contacts --name-query=石 --format=json-minify
cargo run -- get room --room-id 12345678
cargo run -- get room 'https://www.chatwork.com/#!rid12345678'
cargo run -- get room --chat-url 'https://www.chatwork.com/#!rid12345678'
cargo run -- get message --room-id 12345678 --message-id 1234567890123456789
cargo run -- get message 'https://www.chatwork.com/#!rid12345678-1234567890123456789'
cargo run -- get message --chat-url 'https://www.chatwork.com/#!rid12345678-1234567890123456789'
cargo run -- get 'https://www.chatwork.com/#!rid12345678'
cargo run -- get 'https://www.chatwork.com/#!rid12345678-1234567890123456789'
cargo run -- get --chat-url 'https://www.chatwork.com/#!rid12345678'
cargo run -- get --chat-url 'https://www.chatwork.com/#!rid12345678-1234567890123456789'
```

`get me` / `get status` / `get contacts` / `get room` / `get message` は、既定で整形済み JSON を出力します。`--format=json-minify` で 1 行 JSON、`--format=plain` で簡易表示に切り替えられます。

`get my-status` は `get status` の互換名です。

`get contacts` では、次の絞り込みが使えます。

- `--aids=1234567,7654321`: 指定した `account_id` のコンタクトだけ返します。
- `--name-query=石`: `name` を部分一致で絞り込みます。
- `--aids` と `--name-query` は併用できます。

Chatwork URL を `get` の直後または `--chat-url` で渡した場合は、自動で次のように振り分けます。

- `#!rid<room_id>`: `get room`
- `#!rid<room_id>-<message_id>`: `get message`

`get room` / `get message` でも `--chat-url` と位置引数 `[CHAT_URL]` の両方を受け付けますが、同時指定はできません。

### ファイル取得

```bash
cargo run -- download 'https://www.chatwork.com/#!rid12345678-1234567890123456789'
cargo run -- download --chat-url 'https://www.chatwork.com/#!rid12345678-1234567890123456789'
cargo run -- download file 'https://www.chatwork.com/#!rid12345678-1234567890123456789'
cargo run -- download file --chat-url 'https://www.chatwork.com/#!rid12345678-1234567890123456789'
cargo run -- download file --room-id 12345678 --file-id 1234567890
cargo run -- download file --room-id 12345678 --file-id 1234567890 --output ./downloads/report.zip --force
cargo run -- download file --room-id 12345678 --file-id 1234567890 --out-dir ./downloads
```

`download` は、item を省略した場合に暗黙的に `download file` として扱います。

Chatwork のメッセージ URL を位置引数または `--chat-url` で渡すと、メッセージ本文中の `[download:...]` タグから `file_id` を解決してファイルを保存できます。明示的に指定したい場合は、`--room-id` と `--file-id` の組み合わせも使えます。

保存先の優先順位は次のとおりです。

1. `--output`
2. `--out-dir`
3. `CHATWORK_DEFAULT_DOWNLOAD_DIR`
4. カレントディレクトリ

補足は次のとおりです。

- `--output` を省略した場合は、API が返した `filename` をそのまま使います。
- `--output` に既存ディレクトリを指定した場合は、その配下へ `filename` で保存します。
- ディレクトリを明示する場合は `--out-dir` も使えます。
- `--output` と `--out-dir` は同時指定できません。
- 既存ファイルへ上書きする場合は `--force` を付けてください。
- `[download:...]` タグが複数ある場合は、番号、範囲、カンマ区切り、または `A` / `all` で選択できます。
- 空 Enter は `All` 扱いです。

### テンプレート操作

```bash
cargo run -- template list --config ./config/config.example.toml
cargo run -- template show follow_up --config ./config/config.example.toml --var to_id=1234567 --var topic=見積
```

### 送信

```bash
cargo run -- send follow_up --config ./config/config.example.toml --room-id 12345678 --var to_id=1234567 --var topic=見積 --dry-run
cargo run -- send --message '任意の本文です' --room-id 12345678 --dry-run
```

`send` はテンプレート名か `--message` のどちらか一方だけを指定します。

`--message` を使う場合は、`--room-id` を優先し、未指定なら `default_room_id` を使います。`--var` はテンプレート送信時だけ利用できます。

テンプレート送信時の送信先ルームは、次の優先順位で決定されます。

1. `--room-id` (`--room` でも可)
2. テンプレートの `room_id`
3. `default_room_id`

実際に送信する場合は `--dry-run` を外してください。

### サブコマンド短縮

サブコマンドは、他とかぶらない prefix であれば短縮指定できます。

- `chatwork s`: `chatwork send`
- `chatwork d f`: `chatwork download file`
- `chatwork dl`: `chatwork download`

prefix があいまいな場合はエラーになります。

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
