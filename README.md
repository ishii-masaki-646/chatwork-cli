# chatwork-cli

A small Chatwork CLI built with `clap`.

- English: [README.md](./README.md)
- 日本語: [docs/ja/README.md](./docs/ja/README.md)

## Features

- List available templates
- Preview rendered template bodies
- Send rendered template messages to Chatwork
- Send ad-hoc messages without a template
- Fetch your account, status, contacts, rooms, and messages
- Download Chatwork files from IDs or message URLs

## Requirements

- Rust / Cargo
- A Chatwork API token

`CHATWORK_API_TOKEN` is resolved in this order:

1. Regular environment variables
2. `.env` found by searching from the current directory upward
3. `~/.config/chatwork-cli/.env`

```bash
export CHATWORK_API_TOKEN=your_token
```

If you want to use `.env`, use this format:

```dotenv
CHATWORK_API_TOKEN=your_token
# CHATWORK_DEFAULT_DOWNLOAD_DIR=~/Downloads
```

For first-time setup, copy `.env.example` to `.env`. You can also place it at `~/.config/chatwork-cli/.env` as a shared default.

```bash
cp .env.example .env
```

Running `./scripts/build` places a convenient binary at `bin/chatwork`.

## Completion

Use `chatwork completion <shell>` to print completion scripts. The `completion` subcommand works even without a config file.

Template names from the config file are also suggested for `send` and `template show`.

```bash
./bin/chatwork completion bash > ~/.local/share/bash-completion/completions/chatwork
mkdir -p ~/.zfunc
./bin/chatwork completion zsh > ~/.zfunc/_chatwork
```

For `zsh`, add `fpath=(~/.zfunc $fpath)` and `autoload -Uz compinit && compinit` to `~/.zshrc` if needed.

## i18n

Runtime messages are managed with gettext-style `msgid`s. Translation catalogs live under `locale/<lang>/LC_MESSAGES/chatwork-cli.po`.

Japanese is embedded by default. You can switch locales with `CHATWORK_LOCALE` or `LANG`.

If you want to load catalogs from another location, override `CHATWORK_LOCALE_DIR`.

## Configuration

The default config file path is `~/.config/chatwork-cli/config.toml`. See [config/config.example.toml](/home/ishii/work/myrepo/chatwork-cli/config/config.example.toml) for an example.

Template bodies can be written directly in `body` or loaded from files with `body_file`. If `templates_prefix` is omitted, the default is `~/.config/chatwork-cli/templates`. Relative `body_file` paths are resolved from that directory.

```toml
default_room_id = "123456789"
templates_prefix = "~/.config/chatwork-cli/templates"

[templates.follow_up]
description = "Follow-up request"
body_file = "follow_up.txt"

[templates.reminder]
room_id = "987654321"
body = """
[info][title]Reminder[/title]
{{message}}
[/info]
"""
```

## Usage

### Fetch data

```bash
cargo run -- get me
cargo run -- get me --format=plain
cargo run -- get status
cargo run -- get my-status --format=plain
cargo run -- get contacts --format=json-minify
cargo run -- get contacts --aids=123,456 --format=json-minify
cargo run -- get contacts --name-query=ishi --format=json-minify
cargo run -- get room --room-id 123
cargo run -- get room 'https://www.chatwork.com/#!rid123'
cargo run -- get room --chat-url 'https://www.chatwork.com/#!rid123'
cargo run -- get message --room-id 123 --message-id 456
cargo run -- get message 'https://www.chatwork.com/#!rid123-456'
cargo run -- get message --chat-url 'https://www.chatwork.com/#!rid123-456'
cargo run -- get 'https://www.chatwork.com/#!rid123'
cargo run -- get 'https://www.chatwork.com/#!rid123-456'
cargo run -- get --chat-url 'https://www.chatwork.com/#!rid123'
cargo run -- get --chat-url 'https://www.chatwork.com/#!rid123-456'
```

`get me`, `get status`, `get contacts`, `get room`, and `get message` output pretty JSON by default. Use `--format=json-minify` for one-line JSON or `--format=plain` for a compact text view.

`get my-status` is an alias for `get status`.

`get contacts` supports these filters:

- `--aids=123,456`: filter by exact `account_id`
- `--name-query=ishi`: filter by partial `name`
- `--aids` and `--name-query` can be combined

If you pass a Chatwork URL directly to `get` or through `--chat-url`, it is routed automatically:

- `#!rid<room_id>` -> `get room`
- `#!rid<room_id>-<message_id>` -> `get message`

`get room` and `get message` accept both `--chat-url` and positional `[CHAT_URL]`, but not at the same time.

### Download files

```bash
cargo run -- download 'https://www.chatwork.com/#!rid32293227-2090707858361688064'
cargo run -- download --chat-url 'https://www.chatwork.com/#!rid32293227-2090707858361688064'
cargo run -- download file 'https://www.chatwork.com/#!rid32293227-2090707858361688064'
cargo run -- download file --chat-url 'https://www.chatwork.com/#!rid32293227-2090707858361688064'
cargo run -- download file --room-id 123 --file-id 456
cargo run -- download file --room-id 123 --file-id 456 --output ./downloads/report.zip --force
cargo run -- download file --room-id 123 --file-id 456 --out-dir ./downloads
```

If the item is omitted, `download` is treated as `download file`.

When you pass a Chatwork message URL as a positional argument or with `--chat-url`, the CLI resolves `[download:...]` tags from the message body and saves the matched file.

If you want to specify the target directly, use `--room-id` together with `--file-id`.

The output path is resolved in this order:

1. `--output`
2. `--out-dir`
3. `CHATWORK_DEFAULT_DOWNLOAD_DIR`
4. Current directory

Notes:

- If `--output` is omitted, the API `filename` is used as-is.
- If `--output` points to an existing directory, the file is saved there using `filename`.
- Use `--out-dir` when you want to specify a directory explicitly.
- `--output` and `--out-dir` are mutually exclusive.
- Use `--force` to overwrite an existing file.
- If a message contains multiple `[download:...]` tags, you can choose files by number, range, comma-separated list, or `A` / `all`.
- Pressing Enter with no input means `All`.

### Template commands

```bash
cargo run -- template list --config ./config/config.example.toml
cargo run -- template show follow_up --config ./config/config.example.toml --var to_id=12345 --var topic=quote
```

### Send messages

```bash
cargo run -- send follow_up --config ./config/config.example.toml --room-id 123456 --var to_id=12345 --var topic=quote --dry-run
cargo run -- send --message 'Free-form message body' --room-id 123456 --dry-run
```

`send` accepts either a template name or `--message`, but not both.

When using `--message`, `--room-id` takes priority. If it is omitted, `default_room_id` is used. `--var` is available only for template sends.

For template sends, the destination room is resolved in this order:

1. `--room-id` (or `--room`)
2. Template `room_id`
3. `default_room_id`

Remove `--dry-run` for a real send.

### Subcommand shortcuts

Subcommands can be abbreviated as long as the prefix is unique.

- `chatwork s`: `chatwork send`
- `chatwork d f`: `chatwork download file`
- `chatwork dl`: `chatwork download`

Ambiguous prefixes result in an error.

## Build the binary

```bash
./scripts/build
./bin/chatwork template list --config ./config/config.example.toml
```

For a release build:

```bash
./scripts/build --release
./bin/chatwork template list --config ./config/config.example.toml
```
