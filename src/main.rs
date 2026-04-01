use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use dotenvy::{dotenv, from_path};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

mod i18n;
mod shell_completion;

use i18n::{gettext as tr, gettextf as trf};
use shell_completion::ShellScript;

const DEFAULT_BASE_URL: &str = "https://api.chatwork.com/v2";
const TOKEN_ENV_NAME: &str = "CHATWORK_API_TOKEN";
const DEFAULT_DOWNLOAD_DIR_ENV_NAME: &str = "CHATWORK_DEFAULT_DOWNLOAD_DIR";

#[derive(Debug, Parser)]
#[command(name = "chatwork-cli")]
#[command(version, about = "Chatwork の定型文送信を扱う CLI")]
struct Cli {
    /// 設定ファイルのパス
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// 情報を取得する
    Get {
        #[command(subcommand)]
        command: GetCommand,
    },
    /// ファイルをダウンロードする
    Download {
        #[command(subcommand)]
        command: DownloadCommand,
    },
    /// テンプレートを扱う
    Template {
        #[command(subcommand)]
        command: TemplateCommand,
    },
    /// テンプレートを送信する
    Send(SendArgs),
    /// シェル補完スクリプトを出力する
    Completion(CompletionArgs),
    #[command(hide = true, name = "__complete_templates")]
    CompleteTemplates(CompleteTemplatesArgs),
}

#[derive(Debug, Subcommand)]
enum TemplateCommand {
    /// テンプレート一覧を表示する
    List,
    /// テンプレート本文を表示する
    Show(ShowArgs),
}

#[derive(Debug, Subcommand)]
enum GetCommand {
    /// 自分のアカウント情報を表示する
    Me(GetOutputArgs),
    /// 未読やタスクの件数を表示する
    #[command(visible_alias = "my-status")]
    Status(GetOutputArgs),
    /// コンタクト一覧を表示する
    Contacts(GetOutputArgs),
}

#[derive(Debug, Subcommand)]
enum DownloadCommand {
    /// チャットのファイルをダウンロードする
    File(DownloadFileArgs),
}

#[derive(Debug, Args)]
struct GetOutputArgs {
    /// 出力形式
    #[arg(long, value_enum, default_value_t = GetFormat::Json)]
    format: GetFormat,
}

#[derive(Debug, Args)]
struct DownloadFileArgs {
    /// ルーム ID
    #[arg(long, value_name = "ROOM_ID")]
    room_id: Option<u64>,

    /// ファイル ID
    #[arg(long, value_name = "FILE_ID")]
    file_id: Option<u64>,

    /// Chatwork メッセージ URL
    #[arg(long, value_name = "URL")]
    chat_url: Option<String>,

    /// Chatwork メッセージ URL
    #[arg(value_name = "CHAT_URL")]
    chat_url_arg: Option<String>,

    /// 保存先ファイルパス
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,

    /// 保存先ディレクトリ
    #[arg(long, value_name = "DIR")]
    out_dir: Option<PathBuf>,

    /// 既存ファイルを上書きする
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct ShowArgs {
    /// テンプレート名
    name: String,

    /// 差し込み変数。例: --var name=あい
    #[arg(long = "var", value_name = "KEY=VALUE")]
    vars: Vec<String>,
}

#[derive(Debug, Args)]
struct SendArgs {
    /// テンプレート名
    name: String,

    /// 送信先ルーム ID。省略時はテンプレート設定か default_room_id を使う
    #[arg(long, value_name = "ROOM_ID")]
    room: Option<String>,

    /// 差し込み変数。例: --var name=あい
    #[arg(long = "var", value_name = "KEY=VALUE")]
    vars: Vec<String>,

    /// 自分を未読にする
    #[arg(long)]
    self_unread: bool,

    /// 実際には送らず本文だけ表示する
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct CompletionArgs {
    /// 補完スクリプトを生成するシェル
    #[arg(value_enum)]
    shell: CompletionShell,
}

#[derive(Debug, Args)]
struct CompleteTemplatesArgs {
    #[arg(long, default_value = "")]
    current: String,

    #[arg(long)]
    describe: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GetFormat {
    Json,
    JsonMinify,
    Plain,
}

#[derive(Debug, Deserialize, Default)]
struct Config {
    default_room_id: Option<String>,

    #[serde(default = "default_base_url")]
    base_url: String,

    templates_prefix: Option<String>,

    #[serde(default)]
    templates: BTreeMap<String, Template>,

    #[serde(skip)]
    config_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Template {
    description: Option<String>,
    room_id: Option<String>,
    body: Option<String>,
    body_file: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct MeResponse {
    account_id: u64,
    room_id: Option<u64>,
    name: String,
    chatwork_id: String,
    organization_id: Option<u64>,
    organization_name: Option<String>,
    department: Option<String>,
    title: Option<String>,
    url: Option<String>,
    introduction: Option<String>,
    mail: Option<String>,
    tel_organization: Option<String>,
    tel_extension: Option<String>,
    tel_mobile: Option<String>,
    skype: Option<String>,
    facebook: Option<String>,
    twitter: Option<String>,
    avatar_image_url: Option<String>,
    login_mail: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct StatusResponse {
    unread_room_num: u64,
    mention_room_num: u64,
    mytask_room_num: u64,
    unread_num: u64,
    mention_num: u64,
    mytask_num: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct ContactResponse {
    account_id: u64,
    room_id: Option<u64>,
    name: String,
    chatwork_id: String,
    organization_name: Option<String>,
    department: Option<String>,
    avatar_image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RoomFileResponse {
    file_id: u64,
    filename: String,
    download_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RoomMessageResponse {
    body: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DownloadTag {
    file_id: u64,
    label: String,
}

#[derive(Clone, Copy)]
enum CommandContext {
    Root,
    Get,
    Download,
    Template,
    Leaf,
}

#[derive(Clone, Copy, Debug)]
enum UsageContext {
    Root,
    DownloadFile,
    TemplateShow,
    Send,
}

#[derive(Debug)]
struct UsageError {
    context: UsageContext,
    message: String,
}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for UsageError {}

fn default_base_url() -> String {
    DEFAULT_BASE_URL.to_string()
}

impl CompletionShell {
    fn into_shell(self) -> Shell {
        match self {
            Self::Bash => Shell::Bash,
            Self::Elvish => Shell::Elvish,
            Self::Fish => Shell::Fish,
            Self::PowerShell => Shell::PowerShell,
            Self::Zsh => Shell::Zsh,
        }
    }
}

fn main() -> Result<()> {
    load_dotenv_files()?;
    let args = match normalize_cli_args(env::args_os().collect()) {
        Ok(args) => args,
        Err(err) => return handle_cli_error(err),
    };
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => err.exit(),
    };

    let result = match cli.command {
        Commands::Get { command } => {
            handle_get_command(command)
        }
        Commands::Download { command } => {
            handle_download_command(command)
        }
        Commands::Template { command } => {
            let config = load_config_for_cli(cli.config.as_deref())?;
            handle_template_command(command, &config)
        }
        Commands::Send(args) => {
            let config = load_config_for_cli(cli.config.as_deref())?;
            handle_send_command(args, &config)
        }
        Commands::Completion(args) => {
            handle_completion_command(args);
            Ok(())
        }
        Commands::CompleteTemplates(args) => {
            handle_complete_templates_command(args, cli.config.as_deref());
            Ok(())
        }
    };

    match result {
        Ok(()) => Ok(()),
        Err(err) => handle_cli_error(err),
    }
}

fn handle_cli_error(err: anyhow::Error) -> Result<()> {
    if let Some(usage_error) = err.downcast_ref::<UsageError>() {
        eprintln!("Error: {}", usage_error.message);
        eprintln!();
        eprintln!("{}", help_text(usage_error.context));
        std::process::exit(2);
    }

    Err(err)
}

fn usage_error(context: UsageContext, message: impl Into<String>) -> anyhow::Error {
    UsageError {
        context,
        message: message.into(),
    }
    .into()
}

fn help_text(context: UsageContext) -> &'static str {
    match context {
        UsageContext::Root => r#"Usage: chatwork [OPTIONS] <COMMAND>

Commands:
  get         情報を取得する
  download    ファイルをダウンロードする
  template    テンプレートを扱う
  send        テンプレートを送信する
  completion  シェル補完スクリプトを出力する
  help        Print this message or the help of the given subcommand(s)

Options:
      --config <PATH>  設定ファイルのパス
  -h, --help           Print help
  -V, --version        Print version"#,
        UsageContext::DownloadFile => r#"Usage: chatwork download file [OPTIONS] [CHAT_URL]

Arguments:
  [CHAT_URL]  Chatwork メッセージ URL

Options:
      --config <PATH>      設定ファイルのパス
      --room-id <ROOM_ID>  ルーム ID
      --file-id <FILE_ID>  ファイル ID
      --chat-url <URL>     Chatwork メッセージ URL
      --output <PATH>      保存先ファイルパス
      --out-dir <DIR>      保存先ディレクトリ
      --force              既存ファイルを上書きする
  -h, --help               Print help"#,
        UsageContext::TemplateShow => r#"Usage: chatwork template show [OPTIONS] <NAME>

Arguments:
  <NAME>  テンプレート名

Options:
      --config <PATH>    設定ファイルのパス
      --var <KEY=VALUE>  差し込み変数。例: --var name=あい
  -h, --help             Print help"#,
        UsageContext::Send => r#"Usage: chatwork send [OPTIONS] <NAME>

Arguments:
  <NAME>  テンプレート名

Options:
      --config <PATH>    設定ファイルのパス
      --room <ROOM_ID>   送信先ルーム ID。省略時はテンプレート設定か default_room_id を使う
      --var <KEY=VALUE>  差し込み変数。例: --var name=あい
      --self-unread      自分を未読にする
      --dry-run          実際には送らず本文だけ表示する
  -h, --help             Print help"#,
    }
}

fn normalize_cli_args(args: Vec<OsString>) -> Result<Vec<OsString>> {
    if args.len() <= 1 {
        return Ok(args);
    }

    let mut normalized = Vec::with_capacity(args.len());
    let mut context = CommandContext::Root;
    let mut expect_value = false;
    let mut parse_options = true;
    let mut pending_download_default_index = None;
    let mut download_item_seen = false;

    for (index, arg) in args.into_iter().enumerate() {
        if index == 0 {
            normalized.push(arg);
            continue;
        }

        if !parse_options {
            normalized.push(arg);
            continue;
        }

        let Some(text) = arg.to_str().map(str::to_owned) else {
            normalized.push(arg);
            continue;
        };

        if expect_value {
            normalized.push(arg);
            expect_value = false;
            continue;
        }

        if text == "--" {
            normalized.push(arg);
            parse_options = false;
            continue;
        }

        if let Some(long_option) = text.strip_prefix("--") {
            normalized.push(arg);
            if !long_option.contains('=') && long_option_takes_value(long_option) {
                expect_value = true;
            }
            continue;
        }

        if text.starts_with('-') && text != "-" {
            normalized.push(arg);
            continue;
        }

        let resolved = resolve_subcommand_prefix(context, &text)?;
        if let Some(command) = resolved {
            context = next_command_context(context, &command);
            if matches!(context, CommandContext::Download) {
                pending_download_default_index = Some(normalized.len() + 1);
                download_item_seen = false;
            }
            if matches!(command.as_str(), "file" | "help")
                && pending_download_default_index.is_some()
            {
                download_item_seen = true;
            }
            normalized.push(command.into());
            continue;
        }

        if pending_download_default_index.is_some() && !matches!(context, CommandContext::Download) {
            download_item_seen = true;
        }
        normalized.push(arg);
    }

    if let Some(index) = pending_download_default_index {
        if !download_item_seen {
            normalized.insert(index, OsString::from("file"));
        }
    }

    Ok(normalized)
}

fn long_option_takes_value(name: &str) -> bool {
    matches!(
        name,
        "config" | "format" | "chat-url" | "output" | "out-dir" | "room-id" | "file-id" | "room" | "var"
    )
}

fn resolve_subcommand_prefix(context: CommandContext, token: &str) -> Result<Option<String>> {
    if let Some(alias) = resolve_special_subcommand_alias(context, token) {
        return Ok(Some(alias.to_string()));
    }

    let candidates = match context {
        CommandContext::Root => &["get", "download", "template", "send", "completion", "help"][..],
        CommandContext::Get => &["me", "status", "my-status", "contacts", "help"][..],
        CommandContext::Download => &["file", "help"][..],
        CommandContext::Template => &["list", "show", "help"][..],
        CommandContext::Leaf => &[][..],
    };

    if candidates.is_empty() {
        return Ok(None);
    }

    if let Some(exact) = candidates.iter().find(|candidate| **candidate == token) {
        return Ok(Some((*exact).to_string()));
    }

    let matches = candidates
        .iter()
        .filter(|candidate| candidate.starts_with(token))
        .copied()
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [] => Ok(None),
        [matched] => Ok(Some((*matched).to_string())),
        _ => Err(usage_error(
            UsageContext::Root,
            trf(
                "Ambiguous subcommand prefix `{prefix}`: {matches}",
                &[("prefix", token), ("matches", &matches.join(", "))],
            ),
        )),
    }
}

fn resolve_special_subcommand_alias(context: CommandContext, token: &str) -> Option<&'static str> {
    match (context, token) {
        (CommandContext::Root, "dl") => Some("download"),
        _ => None,
    }
}

fn next_command_context(current: CommandContext, command: &str) -> CommandContext {
    match (current, command) {
        (CommandContext::Root, "get") => CommandContext::Get,
        (CommandContext::Root, "download") => CommandContext::Download,
        (CommandContext::Root, "template") => CommandContext::Template,
        _ => CommandContext::Leaf,
    }
}

fn load_dotenv_files() -> Result<()> {
    match dotenv() {
        Ok(_) => Ok(()),
        Err(err) if err.not_found() => {
            if let Some(path) = fallback_dotenv_path() {
                from_path(&path).with_context(|| {
                    trf(
                        "Failed to read dotenv file: {path}",
                        &[("path", &path.display().to_string())],
                    )
                })?;
            }
            Ok(())
        }
        Err(err) => Err(err).context(tr("Failed to load .env file.")),
    }
}

fn handle_completion_command(args: CompletionArgs) {
    match args.shell {
        CompletionShell::Bash => print!("{}", shell_completion::script(ShellScript::Bash)),
        CompletionShell::Zsh => print!("{}", shell_completion::script(ShellScript::Zsh)),
        other => {
            let mut command = Cli::command();
            generate(other.into_shell(), &mut command, "chatwork", &mut io::stdout());
        }
    }
}

fn load_config_for_cli(path: Option<&Path>) -> Result<Config> {
    let config_path = resolve_config_path(path)?;
    load_config(&config_path)
}

fn handle_get_command(command: GetCommand) -> Result<()> {
    match command {
        GetCommand::Me(args) => {
            let token = load_api_token()?;
            let me = get_me(DEFAULT_BASE_URL, &token)?;
            print_me(&me, args.format)?;
        }
        GetCommand::Status(args) => {
            let token = load_api_token()?;
            let status = get_status(DEFAULT_BASE_URL, &token)?;
            print_status(&status, args.format)?;
        }
        GetCommand::Contacts(args) => {
            let token = load_api_token()?;
            let contacts = get_contacts(DEFAULT_BASE_URL, &token)?;
            print_contacts(&contacts, args.format)?;
        }
    }

    Ok(())
}

fn handle_download_command(command: DownloadCommand) -> Result<()> {
    match command {
        DownloadCommand::File(args) => {
            let token = load_api_token()?;
            let files = resolve_download_files(DEFAULT_BASE_URL, &token, &args)?;
            validate_download_destination_args(args.output.as_deref(), args.out_dir.as_deref(), files.len())?;

            for file in files {
                let download_url = file
                    .download_url
                    .as_deref()
                    .context(tr("The response does not contain download_url."))?;
                let output_path = resolve_download_output_path(
                    &file.filename,
                    args.output.as_deref(),
                    args.out_dir.as_deref(),
                );
                ensure_output_writable(&output_path, args.force)?;
                download_to_path(download_url, &output_path)?;
                println!(
                    "{}",
                    trf(
                        "Downloaded the file. file_id={file_id} path={path}",
                        &[
                            ("file_id", &file.file_id.to_string()),
                            ("path", &output_path.display().to_string()),
                        ],
                    )
                );
            }
        }
    }

    Ok(())
}

fn resolve_download_files(base_url: &str, token: &str, args: &DownloadFileArgs) -> Result<Vec<RoomFileResponse>> {
    if args.chat_url.is_some() && args.chat_url_arg.is_some() {
        return Err(usage_error(
            UsageContext::DownloadFile,
            tr("Specify the chat URL either as an argument or with --chat-url, not both."),
        ));
    }

    let chat_url = args.chat_url.as_deref().or(args.chat_url_arg.as_deref());

    match (args.room_id, args.file_id, chat_url) {
        (Some(room_id), Some(file_id), None) => {
            Ok(vec![get_room_file(DEFAULT_BASE_URL, token, room_id, file_id, true)?])
        }
        (None, None, Some(chat_url)) => {
            let (room_id, message_id) = parse_chatwork_message_url(chat_url)?;
            let message = get_room_message(base_url, token, room_id, message_id)?;
            let tags = extract_download_tags(&message.body)?;
            let selected_tags = select_download_tags(&tags)?;

            selected_tags
                .into_iter()
                .map(|tag| get_room_file(DEFAULT_BASE_URL, token, room_id, tag.file_id, true))
                .collect()
        }
        (Some(_), None, None) | (None, Some(_), None) => Err(usage_error(
            UsageContext::DownloadFile,
            tr("Specify both --room-id and --file-id."),
        )),
        _ => Err(usage_error(
            UsageContext::DownloadFile,
            tr("Specify either --chat-url or the pair of --room-id and --file-id."),
        )),
    }
}

fn handle_complete_templates_command(args: CompleteTemplatesArgs, config_path: Option<&Path>) {
    let Some(config) = load_config_for_completion(config_path) else {
        return;
    };

    for (name, template) in &config.templates {
        if name.starts_with(&args.current) {
            if args.describe {
                println!("{name}\t{}", template.description.as_deref().unwrap_or(""));
            } else {
                println!("{name}");
            }
        }
    }
}

fn load_config_for_completion(path: Option<&Path>) -> Option<Config> {
    let config_path = resolve_config_path(path).ok()?;
    load_config(&config_path).ok()
}

fn handle_template_command(command: TemplateCommand, config: &Config) -> Result<()> {
    match command {
        TemplateCommand::List => {
            if config.templates.is_empty() {
                println!("{}", tr("No templates are registered."));
                return Ok(());
            }

            for (name, template) in &config.templates {
                match &template.description {
                    Some(description) => println!("{name}\t{description}"),
                    None => println!("{name}"),
                }
            }
        }
        TemplateCommand::Show(args) => {
            let template = get_template(config, &args.name, UsageContext::TemplateShow)?;
            let body = resolve_template_body(config, &args.name, template, UsageContext::TemplateShow)?;
            let vars = parse_vars(&args.vars, UsageContext::TemplateShow)?;
            let rendered = render_template(&body, &vars, UsageContext::TemplateShow)?;
            println!("{rendered}");
        }
    }

    Ok(())
}

fn handle_send_command(args: SendArgs, config: &Config) -> Result<()> {
    let template = get_template(config, &args.name, UsageContext::Send)?;
    let body = resolve_template_body(config, &args.name, template, UsageContext::Send)?;
    let vars = parse_vars(&args.vars, UsageContext::Send)?;
    let room_id = resolve_room_id(&args, config, template)?;
    let rendered = render_template(&body, &vars, UsageContext::Send)?;

    if args.dry_run {
        println!("{rendered}");
        return Ok(());
    }

    let token = load_api_token()?;

    let message_id = send_message(
        &config.base_url,
        &token,
        &room_id,
        &rendered,
        args.self_unread,
    )?;
    println!(
        "{}",
        trf(
            "Sent the message. room_id={room_id} message_id={message_id}",
            &[("room_id", &room_id), ("message_id", &message_id)],
        )
    );

    Ok(())
}

fn load_api_token() -> Result<String> {
    env::var(TOKEN_ENV_NAME).with_context(|| {
        trf(
            "Set the `{token_env}` environment variable.",
            &[("token_env", TOKEN_ENV_NAME)],
        )
    })
}

fn get_api_json<T>(base_url: &str, token: &str, path: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let endpoint = format!("{}/{}", base_url.trim_end_matches('/'), path.trim_start_matches('/'));
    let client = Client::new();
    let response = client
        .get(endpoint)
        .header("X-ChatWorkToken", token)
        .send()
        .context(tr("Failed to send request to Chatwork API."))?;

    let status = response.status();
    let response_body = response
        .text()
        .context(tr("Failed to read response body from Chatwork API."))?;

    if status != StatusCode::OK {
        bail!(
            "{}",
            trf(
                "Chatwork API returned an error: status={status} body={body}",
                &[("status", status.as_str()), ("body", &response_body)],
            )
        );
    }

    serde_json::from_str(&response_body).context(tr("Failed to parse Chatwork API response JSON."))
}

fn get_room_file(
    base_url: &str,
    token: &str,
    room_id: u64,
    file_id: u64,
    create_download_url: bool,
) -> Result<RoomFileResponse> {
    let endpoint = format!(
        "{}/rooms/{room_id}/files/{file_id}",
        base_url.trim_end_matches('/')
    );
    let client = Client::new();
    let response = client
        .get(endpoint)
        .header("X-ChatWorkToken", token)
        .query(&[("create_download_url", if create_download_url { 1 } else { 0 })])
        .send()
        .context(tr("Failed to send request to Chatwork API."))?;

    let status = response.status();
    let response_body = response
        .text()
        .context(tr("Failed to read response body from Chatwork API."))?;

    if status != StatusCode::OK {
        bail!(
            "{}",
            trf(
                "Chatwork API returned an error: status={status} body={body}",
                &[("status", status.as_str()), ("body", &response_body)],
            )
        );
    }

    serde_json::from_str(&response_body).context(tr("Failed to parse Chatwork API response JSON."))
}

fn get_room_message(base_url: &str, token: &str, room_id: u64, message_id: u64) -> Result<RoomMessageResponse> {
    get_api_json(base_url, token, &format!("/rooms/{room_id}/messages/{message_id}"))
}

fn get_me(base_url: &str, token: &str) -> Result<MeResponse> {
    get_api_json(base_url, token, "/me")
}

fn get_status(base_url: &str, token: &str) -> Result<StatusResponse> {
    get_api_json(base_url, token, "/my/status")
}

fn get_contacts(base_url: &str, token: &str) -> Result<Vec<ContactResponse>> {
    get_api_json(base_url, token, "/contacts")
}

fn parse_chatwork_message_url(url: &str) -> Result<(u64, u64)> {
    let marker = "#!rid";
    let start = url
        .find(marker)
        .ok_or_else(|| usage_error(
            UsageContext::DownloadFile,
            tr("The URL must contain `#!rid<room_id>-<message_id>`."),
        ))?;
    let rest = &url[start + marker.len()..];
    let (room_text, tail) = split_leading_digits(rest)
        .ok_or_else(|| usage_error(UsageContext::DownloadFile, tr("Failed to parse room_id from chat URL.")))?;

    let message_text = tail
        .strip_prefix('-')
        .and_then(|tail| split_leading_digits(tail).map(|(digits, _)| digits))
        .ok_or_else(|| usage_error(UsageContext::DownloadFile, tr("Failed to parse message_id from chat URL.")))?;

    let room_id = room_text
        .parse::<u64>()
        .map_err(|_| usage_error(UsageContext::DownloadFile, tr("Failed to parse room_id from chat URL.")))?;
    let message_id = message_text
        .parse::<u64>()
        .map_err(|_| usage_error(UsageContext::DownloadFile, tr("Failed to parse message_id from chat URL.")))?;

    Ok((room_id, message_id))
}

fn split_leading_digits(text: &str) -> Option<(&str, &str)> {
    let end = text
        .char_indices()
        .find_map(|(idx, ch)| (!ch.is_ascii_digit()).then_some(idx))
        .unwrap_or(text.len());

    if end == 0 {
        None
    } else {
        Some((&text[..end], &text[end..]))
    }
}

fn extract_download_tags(body: &str) -> Result<Vec<DownloadTag>> {
    let mut tags = Vec::new();
    let mut rest = body;
    let marker = "[download:";
    let closing_tag = "[/download]";

    while let Some(start) = rest.find(marker) {
        let after_start = &rest[start + marker.len()..];
        let end = after_start
            .find(']')
            .ok_or_else(|| usage_error(UsageContext::DownloadFile, tr("Missing closing `]` for download tag.")))?;
        let id_text = after_start[..end].trim();
        let file_id = id_text
            .parse::<u64>()
            .map_err(|_| usage_error(
                UsageContext::DownloadFile,
                trf("Failed to parse file_id from download tag: {tag}", &[("tag", id_text)]),
            ))?;
        let after_open_tag = &after_start[end + 1..];
        let close_index = after_open_tag
            .find(closing_tag)
            .ok_or_else(|| usage_error(
                UsageContext::DownloadFile,
                tr("Missing closing `[/download]` for download tag."),
            ))?;
        let label = after_open_tag[..close_index].trim().to_string();
        tags.push(DownloadTag { file_id, label });
        rest = &after_open_tag[close_index + closing_tag.len()..];
    }

    if tags.is_empty() {
        return Err(usage_error(
            UsageContext::DownloadFile,
            tr("The message does not contain a download tag."),
        ));
    }

    Ok(tags)
}

fn select_download_tags(tags: &[DownloadTag]) -> Result<Vec<DownloadTag>> {
    match tags {
        [tag] => Ok(vec![tag.clone()]),
        _ => prompt_download_selection(tags),
    }
}

fn prompt_download_selection(tags: &[DownloadTag]) -> Result<Vec<DownloadTag>> {
    let mut stdout = io::stdout();

    loop {
        writeln!(stdout, "{}", tr("Multiple download tags were found:"))?;
        for (index, tag) in tags.iter().enumerate() {
            writeln!(stdout, "{}. {} (file_id={})", index + 1, tag.label, tag.file_id)?;
        }
        let input = read_selection_line(&mut stdout, &tr("Select numbers, ranges, or [A]ll (default: All):"))?;
        if let Some(selected) = parse_download_selection_input(input.trim(), tags) {
            return Ok(selected);
        }

        writeln!(
            stdout,
            "{}",
            tr("Invalid selection. Enter numbers, ranges, commas, or A.")
        )?;
    }
}

fn read_selection_line(stdout: &mut io::Stdout, prompt: &str) -> Result<String> {
    match DefaultEditor::new() {
        Ok(mut editor) => match editor.readline(&format!("{prompt} ")) {
            Ok(line) => Ok(line),
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => Ok(String::new()),
            Err(_) => read_selection_line_fallback(stdout, prompt),
        },
        Err(_) => read_selection_line_fallback(stdout, prompt),
    }
}

fn read_selection_line_fallback(stdout: &mut io::Stdout, prompt: &str) -> Result<String> {
    write!(stdout, "{prompt} ")?;
    stdout.flush()?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context(tr("Failed to read selection from stdin."))?;
    Ok(input)
}

fn parse_download_selection_input(input: &str, tags: &[DownloadTag]) -> Option<Vec<DownloadTag>> {
    if input.is_empty() || input.eq_ignore_ascii_case("a") || input.eq_ignore_ascii_case("all") {
        return Some(tags.to_vec());
    }

    let mut selected = Vec::new();
    let mut seen = vec![false; tags.len()];

    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return None;
        }

        if let Some((start_text, end_text)) = part.split_once('-') {
            let start = start_text.trim().parse::<usize>().ok()?;
            let end = end_text.trim().parse::<usize>().ok()?;
            if start == 0 || end == 0 || start > end || end > tags.len() {
                return None;
            }

            for index in start..=end {
                if !seen[index - 1] {
                    selected.push(tags[index - 1].clone());
                    seen[index - 1] = true;
                }
            }
            continue;
        }

        let index = part.parse::<usize>().ok()?;
        if index == 0 || index > tags.len() {
            return None;
        }
        if !seen[index - 1] {
            selected.push(tags[index - 1].clone());
            seen[index - 1] = true;
        }
    }

    if selected.is_empty() {
        None
    } else {
        Some(selected)
    }
}

fn validate_download_destination_args(output: Option<&Path>, out_dir: Option<&Path>, file_count: usize) -> Result<()> {
    if output.is_some() && out_dir.is_some() {
        return Err(usage_error(
            UsageContext::DownloadFile,
            tr("Specify either --output or --out-dir, not both."),
        ));
    }

    if file_count > 1 {
        if let Some(path) = output {
            let expanded = expand_home(path);
            if !expanded.is_dir() {
                return Err(usage_error(
                    UsageContext::DownloadFile,
                    tr("Downloading multiple files requires --out-dir, an existing directory passed to --output, or no output path."),
                ));
            }
        }
    }

    Ok(())
}

fn resolve_download_output_path(filename: &str, output: Option<&Path>, out_dir: Option<&Path>) -> PathBuf {
    if let Some(dir) = out_dir {
        return expand_home(dir).join(filename);
    }

    match output {
        Some(path) => {
            let expanded = expand_home(path);
            if expanded.is_dir() {
                expanded.join(filename)
            } else {
                expanded
            }
        }
        None => load_default_download_dir()
            .map(|dir| dir.join(filename))
            .unwrap_or_else(|| PathBuf::from(filename)),
    }
}

fn load_default_download_dir() -> Option<PathBuf> {
    env::var_os(DEFAULT_DOWNLOAD_DIR_ENV_NAME)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| expand_home(&path))
}

fn ensure_output_writable(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "{}",
            trf(
                "Output file already exists. Use --force to overwrite: {path}",
                &[("path", &path.display().to_string())],
            )
        );
    }

    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).with_context(|| {
            trf(
                "Failed to create output directory: {path}",
                &[("path", &parent.display().to_string())],
            )
        })?;
    }

    Ok(())
}

fn download_to_path(download_url: &str, output_path: &Path) -> Result<()> {
    let client = Client::new();
    let response = client
        .get(download_url)
        .send()
        .context(tr("Failed to download file from Chatwork."))?;

    let status = response.status();
    if status != StatusCode::OK {
        let body = response
            .text()
            .unwrap_or_else(|_| String::from("<unavailable>"));
        bail!(
            "{}",
            trf(
                "File download returned an error: status={status} body={body}",
                &[("status", status.as_str()), ("body", &body)],
            )
        );
    }

    let bytes = response
        .bytes()
        .context(tr("Failed to read downloaded file body."))?;
    fs::write(output_path, &bytes).with_context(|| {
        trf(
            "Failed to write downloaded file: {path}",
            &[("path", &output_path.display().to_string())],
        )
    })?;

    Ok(())
}

fn print_json<T>(value: &T, format: GetFormat) -> Result<()>
where
    T: Serialize + ?Sized,
{
    match format {
        GetFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(value)
                    .context(tr("Failed to serialize output as JSON."))?
            );
        }
        GetFormat::JsonMinify => {
            println!(
                "{}",
                serde_json::to_string(value).context(tr("Failed to serialize output as JSON."))?
            );
        }
        GetFormat::Plain => unreachable!("plain format must be handled by caller"),
    }

    Ok(())
}

fn print_me(me: &MeResponse, format: GetFormat) -> Result<()> {
    match format {
        GetFormat::Json | GetFormat::JsonMinify => print_json(me, format)?,
        GetFormat::Plain => {
            println!("account_id={}", me.account_id);
            println!("name={}", me.name);
            println!("chatwork_id={}", me.chatwork_id);

            if let Some(room_id) = me.room_id {
                println!("room_id={room_id}");
            }
            if let Some(organization_id) = me.organization_id {
                println!("organization_id={organization_id}");
            }
            if let Some(organization_name) = &me.organization_name {
                println!("organization_name={organization_name}");
            }
            if let Some(department) = &me.department {
                println!("department={department}");
            }
            if let Some(title) = &me.title {
                println!("title={title}");
            }
            if let Some(url) = &me.url {
                println!("url={url}");
            }
            if let Some(introduction) = &me.introduction {
                println!("introduction={introduction}");
            }
            if let Some(mail) = &me.mail {
                println!("mail={mail}");
            }
            if let Some(login_mail) = &me.login_mail {
                println!("login_mail={login_mail}");
            }
            if let Some(tel_organization) = &me.tel_organization {
                println!("tel_organization={tel_organization}");
            }
            if let Some(tel_extension) = &me.tel_extension {
                println!("tel_extension={tel_extension}");
            }
            if let Some(tel_mobile) = &me.tel_mobile {
                println!("tel_mobile={tel_mobile}");
            }
            if let Some(skype) = &me.skype {
                println!("skype={skype}");
            }
            if let Some(facebook) = &me.facebook {
                println!("facebook={facebook}");
            }
            if let Some(twitter) = &me.twitter {
                println!("twitter={twitter}");
            }
            if let Some(avatar_image_url) = &me.avatar_image_url {
                println!("avatar_image_url={avatar_image_url}");
            }
        }
    }

    Ok(())
}

fn print_status(status: &StatusResponse, format: GetFormat) -> Result<()> {
    match format {
        GetFormat::Json | GetFormat::JsonMinify => print_json(status, format)?,
        GetFormat::Plain => {
            println!("unread_room_num={}", status.unread_room_num);
            println!("mention_room_num={}", status.mention_room_num);
            println!("mytask_room_num={}", status.mytask_room_num);
            println!("unread_num={}", status.unread_num);
            println!("mention_num={}", status.mention_num);
            println!("mytask_num={}", status.mytask_num);
        }
    }

    Ok(())
}

fn print_contacts(contacts: &[ContactResponse], format: GetFormat) -> Result<()> {
    match format {
        GetFormat::Json | GetFormat::JsonMinify => print_json(contacts, format)?,
        GetFormat::Plain => {
            println!("account_id\tchatwork_id\tname\torganization_name\tdepartment\troom_id");
            for contact in contacts {
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    contact.account_id,
                    contact.chatwork_id,
                    contact.name,
                    contact.organization_name.as_deref().unwrap_or(""),
                    contact.department.as_deref().unwrap_or(""),
                    contact.room_id.map(|id| id.to_string()).unwrap_or_default(),
                );
            }
        }
    }

    Ok(())
}

fn get_template<'a>(config: &'a Config, name: &str, context: UsageContext) -> Result<&'a Template> {
    config
        .templates
        .get(name)
        .ok_or_else(|| usage_error(context, trf("Template `{name}` was not found.", &[("name", name)])))
}

fn resolve_template_body(
    config: &Config,
    template_name: &str,
    template: &Template,
    context: UsageContext,
) -> Result<String> {
    match (&template.body, &template.body_file) {
        (Some(body), None) => Ok(body.clone()),
        (None, Some(body_file)) => {
            let path = resolve_template_body_path(config, body_file);
            fs::read_to_string(&path).with_context(|| {
                trf(
                    "Failed to read template body file: {path}",
                    &[("path", &path.display().to_string())],
                )
            })
        }
        _ => Err(usage_error(
            context,
            trf(
                "Template `{name}` must specify exactly one of body or body_file.",
                &[("name", template_name)],
            ),
        )),
    }
}

fn resolve_room_id(args: &SendArgs, config: &Config, template: &Template) -> Result<String> {
    args.room
        .clone()
        .or_else(|| template.room_id.clone())
        .or_else(|| config.default_room_id.clone())
        .ok_or_else(|| usage_error(
            UsageContext::Send,
            tr("Specify one of --room, template room_id, or default_room_id."),
        ))
}

fn parse_vars(items: &[String], context: UsageContext) -> Result<BTreeMap<String, String>> {
    let mut vars = BTreeMap::new();

    for item in items {
        let (key, value) = item
            .split_once('=')
            .ok_or_else(|| usage_error(context, trf("`{item}` must use KEY=VALUE format.", &[("item", item)])))?;
        let key = key.trim();

        if key.is_empty() {
            return Err(usage_error(context, trf("Variable names cannot be empty: `{item}`", &[("item", item)])));
        }

        vars.insert(key.to_string(), value.to_string());
    }

    Ok(vars)
}

fn resolve_template_body_path(config: &Config, body_file: &str) -> PathBuf {
    let path = expand_home(Path::new(body_file));

    if path.is_absolute() {
        return path;
    }

    resolve_templates_prefix(config).join(path)
}

fn resolve_templates_prefix(config: &Config) -> PathBuf {
    match config.templates_prefix.as_deref() {
        Some(prefix) => {
            let path = expand_home(Path::new(prefix));
            if path.is_absolute() {
                path
            } else {
                config.config_dir.join(path)
            }
        }
        None => default_templates_prefix(),
    }
}

fn render_template(body: &str, vars: &BTreeMap<String, String>, context: UsageContext) -> Result<String> {
    let mut rendered = String::with_capacity(body.len());
    let mut rest = body;

    while let Some(start) = rest.find("{{") {
        rendered.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let end = after_start
            .find("}}")
            .ok_or_else(|| usage_error(context, tr("Missing closing `}}` for template placeholder.")))?;
        let key = after_start[..end].trim();

        if key.is_empty() {
            return Err(usage_error(context, tr("Empty placeholder names are not allowed.")));
        }

        let value = vars
            .get(key)
            .ok_or_else(|| usage_error(context, trf("Variable `{key}` is not set.", &[("key", key)])))?;
        rendered.push_str(value);
        rest = &after_start[end + 2..];
    }

    rendered.push_str(rest);
    Ok(rendered)
}

fn resolve_config_path(path: Option<&Path>) -> Result<PathBuf> {
    match path {
        Some(path) => Ok(expand_home(path)),
        None => {
            let home = env::var("HOME").context(tr("HOME is unavailable. Specify `--config`."))?;
            Ok(PathBuf::from(home)
                .join(".config")
                .join("chatwork-cli")
                .join("config.toml"))
        }
    }
}

fn default_templates_prefix() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join("chatwork-cli")
            .join("templates")
    } else {
        PathBuf::from(".config").join("chatwork-cli").join("templates")
    }
}

fn fallback_dotenv_path() -> Option<PathBuf> {
    env::var("HOME").ok().map(|home| default_dotenv_path_for_home(Path::new(&home)))
}

fn default_dotenv_path_for_home(home: &Path) -> PathBuf {
    home.join(".config").join("chatwork-cli").join(".env")
}

fn expand_home(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();

    if text == "~" {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home);
        }
    }

    if let Some(suffix) = text.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return PathBuf::from(home).join(suffix);
        }
    }

    path.to_path_buf()
}

fn load_config(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| trf("Failed to read config file: {path}", &[("path", &path.display().to_string())]))?;
    let mut config: Config = toml::from_str(&content).with_context(|| {
        trf(
            "Failed to parse TOML config: {path}",
            &[("path", &path.display().to_string())],
        )
    })?;

    config.config_dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    if config.templates.is_empty() {
        bail!("{}", tr("At least one template must be defined."));
    }

    validate_templates(&config)?;

    Ok(config)
}

fn validate_templates(config: &Config) -> Result<()> {
    for (name, template) in &config.templates {
        match (&template.body, &template.body_file) {
            (Some(_), None) | (None, Some(_)) => {}
            _ => {
                bail!(
                    "{}",
                    trf(
                        "Template `{name}` must specify exactly one of body or body_file.",
                        &[("name", name)],
                    )
                )
            }
        }
    }

    Ok(())
}

fn send_message(
    base_url: &str,
    token: &str,
    room_id: &str,
    body: &str,
    self_unread: bool,
) -> Result<String> {
    let endpoint = format!(
        "{}/rooms/{}/messages",
        base_url.trim_end_matches('/'),
        room_id
    );

    let client = Client::new();
    let response = client
        .post(endpoint)
        .header("X-ChatWorkToken", token)
        .form(&[
            ("body", body.to_string()),
            (
                "self_unread",
                if self_unread { "1" } else { "0" }.to_string(),
            ),
        ])
        .send()
        .context(tr("Failed to send request to Chatwork API."))?;

    let status = response.status();
    let response_body = response
        .text()
        .context(tr("Failed to read response body from Chatwork API."))?;

    if status != StatusCode::OK {
        bail!(
            "{}",
            trf(
                "Chatwork API returned an error: status={status} body={body}",
                &[("status", status.as_str()), ("body", &response_body)],
            )
        );
    }

    let message_id = extract_message_id(&response_body)?;
    Ok(message_id)
}

fn extract_message_id(response_body: &str) -> Result<String> {
    let prefix = "\"message_id\":\"";
    let start = response_body
        .find(prefix)
        .context(tr("The response does not contain message_id."))?;
    let rest = &response_body[start + prefix.len()..];
    let end = rest
        .find('"')
        .context(tr("Failed to parse message_id from the response."))?;
    Ok(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn render_template_replaces_placeholders() {
        let vars = BTreeMap::from([
            ("name".to_string(), "あい".to_string()),
            ("topic".to_string(), "定型文".to_string()),
        ]);
        let rendered = render_template("{{name}} が {{topic}} を送る", &vars, UsageContext::Send).unwrap();
        assert_eq!(rendered, "あい が 定型文 を送る");
    }

    #[test]
    fn render_template_fails_on_missing_value() {
        let vars = BTreeMap::new();
        let err = render_template("{{name}}", &vars, UsageContext::Send).unwrap_err();
        assert_eq!(
            err.to_string(),
            trf("Variable `{key}` is not set.", &[("key", "name")]),
        );
    }

    #[test]
    fn parse_vars_reads_key_value_pairs() {
        let vars = parse_vars(
            &["name=あい".to_string(), "topic=確認".to_string()],
            UsageContext::Send,
        )
        .unwrap();
        assert_eq!(vars.get("name").unwrap(), "あい");
        assert_eq!(vars.get("topic").unwrap(), "確認");
    }

    #[test]
    fn extract_message_id_reads_response_json() {
        let message_id = extract_message_id(r#"{"message_id":"12345"}"#).unwrap();
        assert_eq!(message_id, "12345");
    }

    #[test]
    fn get_command_parses_without_config() {
        let cli = Cli::try_parse_from(["chatwork", "get", "me"]).unwrap();

        match cli.command {
            Commands::Get { command } => {
                assert!(matches!(command, GetCommand::Me(GetOutputArgs { format: GetFormat::Json })));
            }
            _ => panic!("get command was not parsed"),
        }
    }

    #[test]
    fn get_command_parses_plain_format() {
        let cli = Cli::try_parse_from(["chatwork", "get", "me", "--format=plain"]).unwrap();

        match cli.command {
            Commands::Get { command } => {
                assert!(matches!(command, GetCommand::Me(GetOutputArgs { format: GetFormat::Plain })));
            }
            _ => panic!("get command was not parsed"),
        }
    }

    #[test]
    fn get_status_command_parses_without_config() {
        let cli = Cli::try_parse_from(["chatwork", "get", "status"]).unwrap();

        match cli.command {
            Commands::Get { command } => {
                assert!(matches!(command, GetCommand::Status(GetOutputArgs { format: GetFormat::Json })));
            }
            _ => panic!("get status command was not parsed"),
        }
    }

    #[test]
    fn get_contacts_command_parses_plain_format() {
        let cli = Cli::try_parse_from(["chatwork", "get", "contacts", "--format=plain"]).unwrap();

        match cli.command {
            Commands::Get { command } => {
                assert!(matches!(command, GetCommand::Contacts(GetOutputArgs { format: GetFormat::Plain })));
            }
            _ => panic!("get command was not parsed"),
        }
    }

    #[test]
    fn get_my_status_alias_parses_without_config() {
        let cli = Cli::try_parse_from(["chatwork", "get", "my-status"]).unwrap();

        match cli.command {
            Commands::Get { command } => {
                assert!(matches!(command, GetCommand::Status(GetOutputArgs { format: GetFormat::Json })));
            }
            _ => panic!("get my-status command was not parsed"),
        }
    }

    #[test]
    fn normalize_cli_args_expands_unique_subcommand_prefixes() {
        let args = normalize_cli_args(vec![
            "chatwork".into(),
            "d".into(),
            "f".into(),
            "--chat-url".into(),
            "https://www.chatwork.com/#!rid1-2".into(),
        ])
        .unwrap();

        assert_eq!(
            args,
            vec![
                OsString::from("chatwork"),
                OsString::from("download"),
                OsString::from("file"),
                OsString::from("--chat-url"),
                OsString::from("https://www.chatwork.com/#!rid1-2"),
            ]
        );
    }

    #[test]
    fn normalize_cli_args_expands_special_download_alias() {
        let args = normalize_cli_args(vec![
            "chatwork".into(),
            "dl".into(),
            "f".into(),
        ])
        .unwrap();

        assert_eq!(
            args,
            vec![
                OsString::from("chatwork"),
                OsString::from("download"),
                OsString::from("file"),
            ]
        );
    }

    #[test]
    fn normalize_cli_args_inserts_default_download_file_subcommand() {
        let args = normalize_cli_args(vec![
            "chatwork".into(),
            "download".into(),
            "--chat-url".into(),
            "https://www.chatwork.com/#!rid1-2".into(),
        ])
        .unwrap();

        assert_eq!(
            args,
            vec![
                OsString::from("chatwork"),
                OsString::from("download"),
                OsString::from("file"),
                OsString::from("--chat-url"),
                OsString::from("https://www.chatwork.com/#!rid1-2"),
            ]
        );
    }

    #[test]
    fn normalize_cli_args_inserts_default_download_file_subcommand_for_dl_alias() {
        let args = normalize_cli_args(vec![
            "chatwork".into(),
            "dl".into(),
            "https://www.chatwork.com/#!rid1-2".into(),
        ])
        .unwrap();

        assert_eq!(
            args,
            vec![
                OsString::from("chatwork"),
                OsString::from("download"),
                OsString::from("file"),
                OsString::from("https://www.chatwork.com/#!rid1-2"),
            ]
        );
    }

    #[test]
    fn normalize_cli_args_expands_nested_subcommand_prefix_after_global_option() {
        let args = normalize_cli_args(vec![
            "chatwork".into(),
            "--config".into(),
            "config.toml".into(),
            "g".into(),
            "s".into(),
        ])
        .unwrap();

        assert_eq!(
            args,
            vec![
                OsString::from("chatwork"),
                OsString::from("--config"),
                OsString::from("config.toml"),
                OsString::from("get"),
                OsString::from("status"),
            ]
        );
    }

    #[test]
    fn normalize_cli_args_rejects_ambiguous_subcommand_prefix() {
        let err = normalize_cli_args(vec![
            "chatwork".into(),
            "g".into(),
            "m".into(),
        ])
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            trf(
                "Ambiguous subcommand prefix `{prefix}`: {matches}",
                &[("prefix", "m"), ("matches", "me, my-status")],
            )
        );
    }

    #[test]
    fn download_file_command_parses_without_config() {
        let cli = Cli::try_parse_from([
            "chatwork",
            "download",
            "file",
            "--room-id",
            "123",
            "--file-id",
            "456",
        ])
        .unwrap();

        match cli.command {
            Commands::Download { command } => match command {
                DownloadCommand::File(args) => {
                    assert_eq!(args.room_id, Some(123));
                    assert_eq!(args.file_id, Some(456));
                    assert_eq!(args.chat_url, None);
                    assert_eq!(args.chat_url_arg, None);
                    assert_eq!(args.output, None);
                    assert_eq!(args.out_dir, None);
                    assert!(!args.force);
                }
            },
            _ => panic!("download file command was not parsed"),
        }
    }

    #[test]
    fn resolve_download_output_path_uses_filename_by_default() {
        env::remove_var(DEFAULT_DOWNLOAD_DIR_ENV_NAME);
        let path = resolve_download_output_path("report.txt", None, None);
        assert_eq!(path, Path::new("report.txt"));
    }

    #[test]
    fn download_file_command_parses_chat_url_as_positional_argument() {
        let cli = Cli::try_parse_from([
            "chatwork",
            "download",
            "file",
            "https://www.chatwork.com/#!rid32293227-2090707858361688064",
        ])
        .unwrap();

        match cli.command {
            Commands::Download { command } => match command {
                DownloadCommand::File(args) => {
                    assert_eq!(args.room_id, None);
                    assert_eq!(args.file_id, None);
                    assert_eq!(args.chat_url, None);
                    assert_eq!(
                        args.chat_url_arg.as_deref(),
                        Some("https://www.chatwork.com/#!rid32293227-2090707858361688064")
                    );
                }
            },
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn resolve_download_files_rejects_both_chat_url_forms() {
        let err = resolve_download_files(
            DEFAULT_BASE_URL,
            "dummy-token",
            &DownloadFileArgs {
                room_id: None,
                file_id: None,
                chat_url: Some("https://www.chatwork.com/#!rid1-2".to_string()),
                chat_url_arg: Some("https://www.chatwork.com/#!rid1-2".to_string()),
                output: None,
                out_dir: None,
                force: false,
            },
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            tr("Specify the chat URL either as an argument or with --chat-url, not both.")
        );
    }

    #[test]
    fn resolve_download_files_rejects_chat_url_with_room_and_file_ids() {
        let err = resolve_download_files(
            DEFAULT_BASE_URL,
            "dummy-token",
            &DownloadFileArgs {
                room_id: Some(123),
                file_id: Some(456),
                chat_url: None,
                chat_url_arg: Some("https://www.chatwork.com/#!rid1-2".to_string()),
                output: None,
                out_dir: None,
                force: false,
            },
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            tr("Specify either --chat-url or the pair of --room-id and --file-id.")
        );
    }

    #[test]
    fn resolve_download_output_path_uses_default_download_dir_from_env() {
        env::set_var(DEFAULT_DOWNLOAD_DIR_ENV_NAME, "~/Downloads");
        env::set_var("HOME", "/tmp/chatwork-cli-home");

        let path = resolve_download_output_path("report.txt", None, None);

        assert_eq!(path, Path::new("/tmp/chatwork-cli-home/Downloads/report.txt"));

        env::remove_var(DEFAULT_DOWNLOAD_DIR_ENV_NAME);
    }

    #[test]
    fn parse_chatwork_message_url_reads_room_and_message_ids() {
        let (room_id, message_id) =
            parse_chatwork_message_url("https://www.chatwork.com/#!rid32293227-2090707858361688064")
                .unwrap();
        assert_eq!(room_id, 32293227);
        assert_eq!(message_id, 2090707858361688064);
    }

    #[test]
    fn extract_download_tags_reads_single_tag() {
        let tags = extract_download_tags(
            "[info][download:2019373427]file.zip (1 KB)[/download][/info]",
        )
        .unwrap();
        assert_eq!(
            tags,
            vec![DownloadTag {
                file_id: 2019373427,
                label: "file.zip (1 KB)".to_string(),
            }]
        );
    }

    #[test]
    fn extract_download_tags_reads_multiple_tags() {
        let tags = extract_download_tags(
            "[download:1]a.zip[/download]\n[download:2]b.zip[/download]",
        )
        .unwrap();
        assert_eq!(
            tags,
            vec![
                DownloadTag {
                    file_id: 1,
                    label: "a.zip".to_string(),
                },
                DownloadTag {
                    file_id: 2,
                    label: "b.zip".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_download_selection_input_uses_all_for_empty_input() {
        let tags = vec![
            DownloadTag {
                file_id: 1,
                label: "a.zip".to_string(),
            },
            DownloadTag {
                file_id: 2,
                label: "b.zip".to_string(),
            },
        ];

        assert_eq!(
            parse_download_selection_input("", &tags),
            Some(tags),
        );
    }

    #[test]
    fn parse_download_selection_input_reads_single_number() {
        let tags = vec![
            DownloadTag {
                file_id: 1,
                label: "a.zip".to_string(),
            },
            DownloadTag {
                file_id: 2,
                label: "b.zip".to_string(),
            },
        ];

        assert_eq!(
            parse_download_selection_input("2", &tags),
            Some(vec![tags[1].clone()]),
        );
    }

    #[test]
    fn parse_download_selection_input_reads_ranges_and_lists() {
        let tags = vec![
            DownloadTag {
                file_id: 1,
                label: "a.zip".to_string(),
            },
            DownloadTag {
                file_id: 2,
                label: "b.zip".to_string(),
            },
            DownloadTag {
                file_id: 3,
                label: "c.zip".to_string(),
            },
            DownloadTag {
                file_id: 4,
                label: "d.zip".to_string(),
            },
        ];

        assert_eq!(
            parse_download_selection_input("1,3-4", &tags),
            Some(vec![tags[0].clone(), tags[2].clone(), tags[3].clone()]),
        );
        assert_eq!(
            parse_download_selection_input("2-3", &tags),
            Some(vec![tags[1].clone(), tags[2].clone()]),
        );
    }

    #[test]
    fn parse_download_selection_input_deduplicates_indices() {
        let tags = vec![
            DownloadTag {
                file_id: 1,
                label: "a.zip".to_string(),
            },
            DownloadTag {
                file_id: 2,
                label: "b.zip".to_string(),
            },
            DownloadTag {
                file_id: 3,
                label: "c.zip".to_string(),
            },
        ];

        assert_eq!(
            parse_download_selection_input("1,1-2,2", &tags),
            Some(vec![tags[0].clone(), tags[1].clone()]),
        );
    }

    #[test]
    fn parse_download_selection_input_rejects_invalid_ranges() {
        let tags = vec![
            DownloadTag {
                file_id: 1,
                label: "a.zip".to_string(),
            },
            DownloadTag {
                file_id: 2,
                label: "b.zip".to_string(),
            },
        ];

        assert_eq!(parse_download_selection_input("0", &tags), None);
        assert_eq!(parse_download_selection_input("3", &tags), None);
        assert_eq!(parse_download_selection_input("2-1", &tags), None);
        assert_eq!(parse_download_selection_input("1,", &tags), None);
        assert_eq!(parse_download_selection_input("1-a", &tags), None);
    }

    #[test]
    fn resolve_download_output_path_expands_home() {
        env::set_var("HOME", "/tmp/chatwork-cli-home");
        let path = resolve_download_output_path("report.txt", Some(Path::new("~/Downloads/report.txt")), None);
        assert_eq!(path, Path::new("/tmp/chatwork-cli-home/Downloads/report.txt"));
    }

    #[test]
    fn resolve_download_output_path_uses_filename_when_output_is_directory() {
        let dir = temp_test_dir("resolve_download_output_path_uses_filename_when_output_is_directory");
        fs::create_dir_all(&dir).unwrap();
        let path = resolve_download_output_path("report.txt", Some(&dir), None);
        assert_eq!(path, dir.join("report.txt"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resolve_download_output_path_uses_out_dir() {
        let path = resolve_download_output_path("report.txt", None, Some(Path::new("/tmp/downloads")));
        assert_eq!(path, Path::new("/tmp/downloads/report.txt"));
    }

    #[test]
    fn validate_download_destination_args_rejects_both_output_and_out_dir() {
        let err = validate_download_destination_args(
            Some(Path::new("/tmp/report.txt")),
            Some(Path::new("/tmp/downloads")),
            1,
        )
        .unwrap_err();
        assert_eq!(err.to_string(), tr("Specify either --output or --out-dir, not both."));
    }

    #[test]
    fn validate_download_destination_args_rejects_single_output_path_for_multiple_files() {
        let err =
            validate_download_destination_args(Some(Path::new("/tmp/report.txt")), None, 2).unwrap_err();
        assert_eq!(
            err.to_string(),
            tr("Downloading multiple files requires --out-dir, an existing directory passed to --output, or no output path."),
        );
    }

    #[test]
    fn default_dotenv_path_for_home_uses_config_directory() {
        let path = default_dotenv_path_for_home(Path::new("/tmp/example-home"));
        assert_eq!(path, Path::new("/tmp/example-home/.config/chatwork-cli/.env"));
    }

    #[test]
    fn load_config_parses_templates() {
        let dir = temp_test_dir("load_config_parses_templates");
        let templates_dir = dir.join("templates");
        fs::create_dir_all(&templates_dir).unwrap();
        fs::write(templates_dir.join("greeting.txt"), "こんにちは、{{name}}さん\n").unwrap();
        let config_path = dir.join("config.toml");
        fs::write(
            &config_path,
            r#"
default_room_id = "123456"
templates_prefix = "./templates"

[templates.greeting]
description = "あいさつ"
body_file = "greeting.txt"
"#,
        )
        .unwrap();

        let config = load_config(&config_path).unwrap();
        let template = config.templates.get("greeting").unwrap();

        assert_eq!(config.default_room_id.as_deref(), Some("123456"));
        assert_eq!(config.templates_prefix.as_deref(), Some("./templates"));
        assert_eq!(template.description.as_deref(), Some("あいさつ"));
        assert_eq!(
            resolve_template_body(&config, "greeting", template, UsageContext::TemplateShow).unwrap(),
            "こんにちは、{{name}}さん\n"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_config_rejects_multiple_body_sources() {
        let dir = temp_test_dir("load_config_rejects_multiple_body_sources");
        let config_path = dir.join("config.toml");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &config_path,
            r#"
[templates.invalid]
body = "inline"
body_file = "invalid.txt"
"#,
        )
        .unwrap();

        let err = load_config(&config_path).unwrap_err();
        assert_eq!(
            err.to_string(),
            trf(
                "Template `{name}` must specify exactly one of body or body_file.",
                &[("name", "invalid")],
            )
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn completion_command_parses_without_config() {
        let cli = Cli::try_parse_from(["chatwork", "completion", "bash"]).unwrap();

        match cli.command {
            Commands::Completion(args) => {
                assert!(matches!(args.shell, CompletionShell::Bash));
            }
            _ => panic!("completion command was not parsed"),
        }
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("chatwork-cli-{name}-{unique}"))
    }
}
