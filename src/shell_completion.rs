pub fn script(shell: ShellScript) -> &'static str {
    match shell {
        ShellScript::Bash => BASH_SCRIPT,
        ShellScript::Zsh => ZSH_SCRIPT,
    }
}

#[derive(Clone, Copy)]
pub enum ShellScript {
    Bash,
    Zsh,
}

const BASH_SCRIPT: &str = r#"_chatwork_resolve_prefix() {
    local context="$1"
    local token="$2"
    local -a candidates matches
    local candidate

    if [[ "${context}" == "root" && "${token}" == "dl" ]]; then
        printf '%s\n' "download"
        return 0
    fi

    case "${context}" in
        root)
            candidates=(get download template send completion help)
            ;;
        get)
            candidates=(me status my-status contacts room message help)
            ;;
        download)
            candidates=(file help)
            ;;
        template)
            candidates=(list show help)
            ;;
        *)
            return 0
            ;;
    esac

    for candidate in "${candidates[@]}"; do
        if [[ "${candidate}" == "${token}" ]]; then
            printf '%s\n' "${candidate}"
            return 0
        fi
    done

    for candidate in "${candidates[@]}"; do
        if [[ "${candidate}" == "${token}"* ]]; then
            matches+=("${candidate}")
        fi
    done

    if (( ${#matches[@]} == 1 )); then
        printf '%s\n' "${matches[0]}"
    fi
}

_chatwork() {
    local cur prev
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev=""
    if (( COMP_CWORD > 0 )); then
        prev="${COMP_WORDS[COMP_CWORD-1]}"
    fi

    local config=""
    local download_subcmd=""
    local mode=""
    local get_subcmd=""
    local template_subcmd=""
    local positional_seen=0
    local i word resolved context

    for ((i=1; i<COMP_CWORD; i++)); do
        word="${COMP_WORDS[i]}"
        context="${mode}"
        if [[ -z "${context}" ]]; then
            context="root"
        fi
        if [[ "${word}" != --* && "${word}" != -* && "${word}" != "-" ]]; then
            resolved="$(_chatwork_resolve_prefix "${context}" "${word}")"
            if [[ -n "${resolved}" ]]; then
                word="${resolved}"
            fi
        fi
        case "${word}" in
            --config)
                if (( i + 1 < COMP_CWORD )); then
                    ((i++))
                    config="${COMP_WORDS[i]}"
                fi
                ;;
            --format)
                if (( i + 1 < COMP_CWORD )); then
                    ((i++))
                fi
                ;;
            --chat-url|--output|--out-dir|--room-id|--file-id|--message-id)
                if (( i + 1 < COMP_CWORD )); then
                    ((i++))
                fi
                ;;
            download)
                mode="download"
                download_subcmd=""
                positional_seen=0
                ;;
            get)
                mode="get"
                get_subcmd=""
                positional_seen=0
                ;;
            template)
                mode="template"
                template_subcmd=""
                positional_seen=0
                ;;
            send)
                mode="send"
                positional_seen=0
                ;;
            me)
                if [[ "${mode}" == "get" ]]; then
                    get_subcmd="me"
                fi
                ;;
            status|my-status)
                if [[ "${mode}" == "get" ]]; then
                    get_subcmd="status"
                fi
                ;;
            contacts)
                if [[ "${mode}" == "get" ]]; then
                    get_subcmd="contacts"
                fi
                ;;
            room)
                if [[ "${mode}" == "get" ]]; then
                    get_subcmd="room"
                fi
                ;;
            message)
                if [[ "${mode}" == "get" ]]; then
                    get_subcmd="message"
                fi
                ;;
            file)
                if [[ "${mode}" == "download" ]]; then
                    download_subcmd="file"
                fi
                ;;
            show)
                if [[ "${mode}" == "template" ]]; then
                    template_subcmd="show"
                    positional_seen=0
                fi
                ;;
            list)
                if [[ "${mode}" == "template" ]]; then
                    template_subcmd="list"
                fi
                ;;
            completion)
                mode="completion"
                ;;
            --room|--var)
                if (( i + 1 < COMP_CWORD )); then
                    ((i++))
                fi
                ;;
            --*)
                ;;
            *)
                if [[ "${mode}" == "send" && ${positional_seen} -eq 0 ]]; then
                    positional_seen=1
                elif [[ "${mode}" == "template" && "${template_subcmd}" == "show" && ${positional_seen} -eq 0 ]]; then
                    positional_seen=1
                fi
                ;;
        esac
    done

    case "${prev}" in
        --config)
            COMPREPLY=( $(compgen -f -- "${cur}") )
            return 0
            ;;
        --format)
            COMPREPLY=( $(compgen -W "json json-minify plain" -- "${cur}") )
            return 0
            ;;
        --output|--out-dir)
            COMPREPLY=( $(compgen -f -- "${cur}") )
            return 0
            ;;
        --chat-url)
            return 0
            ;;
        --room-id|--file-id|--message-id|--room|--var)
            return 0
            ;;
    esac

    if [[ "${mode}" == "send" ]]; then
        local opts="--room --var --self-unread --dry-run --config --help"
        if [[ ${positional_seen} -eq 0 && "${cur}" != -* ]]; then
            local cmd=(chatwork)
            local templates combined
            if [[ -n "${config}" ]]; then
                cmd+=(--config "${config}")
            fi
            templates=$("${cmd[@]}" __complete_templates --current "${cur}" 2>/dev/null)
            combined="${templates} ${opts}"
            COMPREPLY=( $(compgen -W "${combined}" -- "${cur}") )
            return 0
        fi
        COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "get" && ( "${get_subcmd}" == "me" || "${get_subcmd}" == "status" || "${get_subcmd}" == "contacts" ) ]]; then
        COMPREPLY=( $(compgen -W "--format --config --help" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "get" && "${get_subcmd}" == "room" ]]; then
        COMPREPLY=( $(compgen -W "--room-id --chat-url --format --config --help" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "get" && "${get_subcmd}" == "message" ]]; then
        COMPREPLY=( $(compgen -W "--room-id --message-id --chat-url --format --config --help" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "get" && -z "${get_subcmd}" ]]; then
        COMPREPLY=( $(compgen -W "me status my-status contacts room message --config --help" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "download" && "${download_subcmd}" == "file" ]]; then
        COMPREPLY=( $(compgen -W "--chat-url --room-id --file-id --output --out-dir --force --config --help" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "download" && -z "${download_subcmd}" ]]; then
        COMPREPLY=( $(compgen -W "--chat-url --room-id --file-id --output --out-dir --force --config --help" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "template" && "${template_subcmd}" == "show" ]]; then
        local opts="--var --config --help"
        if [[ ${positional_seen} -eq 0 && "${cur}" != -* ]]; then
            local cmd=(chatwork)
            local templates combined
            if [[ -n "${config}" ]]; then
                cmd+=(--config "${config}")
            fi
            templates=$("${cmd[@]}" __complete_templates --current "${cur}" 2>/dev/null)
            combined="${templates} ${opts}"
            COMPREPLY=( $(compgen -W "${combined}" -- "${cur}") )
            return 0
        fi
        COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "template" && "${template_subcmd}" == "list" ]]; then
        COMPREPLY=( $(compgen -W "--config --help" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "template" && -z "${template_subcmd}" ]]; then
        COMPREPLY=( $(compgen -W "list show --config --help" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "completion" ]]; then
        COMPREPLY=( $(compgen -W "bash zsh fish elvish power-shell --config --help" -- "${cur}") )
        return 0
    fi

    COMPREPLY=( $(compgen -W "get download template send completion --config --help --version -h -V" -- "${cur}") )
    return 0
}

complete -F _chatwork chatwork
"#;

const ZSH_SCRIPT: &str = r#"#compdef chatwork

_chatwork_resolve_prefix() {
    local context="$1"
    local token="$2"
    local -a candidates matches
    local candidate

    if [[ "${context}" == "root" && "${token}" == "dl" ]]; then
        print -r -- "download"
        return 0
    fi

    case "${context}" in
        root)
            candidates=(get download template send completion help)
            ;;
        get)
            candidates=(me status my-status contacts room message help)
            ;;
        download)
            candidates=(file help)
            ;;
        template)
            candidates=(list show help)
            ;;
        *)
            return 0
            ;;
    esac

    for candidate in "${candidates[@]}"; do
        if [[ "${candidate}" == "${token}" ]]; then
            print -r -- "${candidate}"
            return 0
        fi
    done

    for candidate in "${candidates[@]}"; do
        if [[ "${candidate}" == "${token}"* ]]; then
            matches+=("${candidate}")
        fi
    done

    if (( ${#matches[@]} == 1 )); then
        print -r -- "${matches[1]}"
    fi
}

_chatwork_add_described() {
    local -a matches display_strings
    local spec match description

    for spec in "$@"; do
        match="${spec%%$'\t'*}"
        description="${spec#*$'\t'}"
        matches+=("${match}")
        if [[ "${description}" == "${spec}" || -z "${description}" ]]; then
            display_strings+=("${match}")
        else
            display_strings+=("${match} -- ${description}")
        fi
    done

    (( ${#matches[@]} > 0 )) && compadd -l -o match -d display_strings -- "${matches[@]}"
}

_chatwork_add_described_group() {
    local group_name="$1"
    local header="$2"
    shift 2

    local -a matches display_strings
    local spec match description

    for spec in "$@"; do
        match="${spec%%$'\t'*}"
        description="${spec#*$'\t'}"
        matches+=("${match}")
        if [[ "${description}" == "${spec}" || -z "${description}" ]]; then
            display_strings+=("${match}")
        else
            display_strings+=("${match} -- ${description}")
        fi
    done

    (( ${#matches[@]} > 0 )) && compadd -V "${group_name}" -X "${header}" -l -o match -d display_strings -- "${matches[@]}"
}

_chatwork() {
    local cur prev
    cur="${words[CURRENT]}"
    prev=""
    if (( CURRENT > 1 )); then
        prev="${words[CURRENT-1]}"
    fi

    local config=""
    local download_subcmd=""
    local mode=""
    local get_subcmd=""
    local template_subcmd=""
    local positional_seen=0
    local i word resolved context

    for ((i=2; i<CURRENT; i++)); do
        word="${words[i]}"
        context="${mode}"
        if [[ -z "${context}" ]]; then
            context="root"
        fi
        if [[ "${word}" != --* && "${word}" != -* && "${word}" != "-" ]]; then
            resolved="$(_chatwork_resolve_prefix "${context}" "${word}")"
            if [[ -n "${resolved}" ]]; then
                word="${resolved}"
            fi
        fi
        case "${word}" in
            --config)
                if (( i + 1 < CURRENT )); then
                    ((i++))
                    config="${words[i]}"
                fi
                ;;
            --format)
                if (( i + 1 < CURRENT )); then
                    ((i++))
                fi
                ;;
            --chat-url|--output|--out-dir|--room-id|--file-id|--message-id)
                if (( i + 1 < CURRENT )); then
                    ((i++))
                fi
                ;;
            download)
                mode="download"
                download_subcmd=""
                positional_seen=0
                ;;
            get)
                mode="get"
                get_subcmd=""
                positional_seen=0
                ;;
            template)
                mode="template"
                template_subcmd=""
                positional_seen=0
                ;;
            send)
                mode="send"
                positional_seen=0
                ;;
            me)
                if [[ "${mode}" == "get" ]]; then
                    get_subcmd="me"
                fi
                ;;
            status|my-status)
                if [[ "${mode}" == "get" ]]; then
                    get_subcmd="status"
                fi
                ;;
            contacts)
                if [[ "${mode}" == "get" ]]; then
                    get_subcmd="contacts"
                fi
                ;;
            room)
                if [[ "${mode}" == "get" ]]; then
                    get_subcmd="room"
                fi
                ;;
            message)
                if [[ "${mode}" == "get" ]]; then
                    get_subcmd="message"
                fi
                ;;
            file)
                if [[ "${mode}" == "download" ]]; then
                    download_subcmd="file"
                fi
                ;;
            show)
                if [[ "${mode}" == "template" ]]; then
                    template_subcmd="show"
                    positional_seen=0
                fi
                ;;
            list)
                if [[ "${mode}" == "template" ]]; then
                    template_subcmd="list"
                fi
                ;;
            completion)
                mode="completion"
                ;;
            --room|--var)
                if (( i + 1 < CURRENT )); then
                    ((i++))
                fi
                ;;
            --*)
                ;;
            *)
                if [[ "${mode}" == "send" && ${positional_seen} -eq 0 ]]; then
                    positional_seen=1
                elif [[ "${mode}" == "template" && "${template_subcmd}" == "show" && ${positional_seen} -eq 0 ]]; then
                    positional_seen=1
                fi
                ;;
        esac
    done

    case "${prev}" in
        --config)
            _files
            return 0
            ;;
        --format)
            local -a formats
            formats=(
                $'json\t整形済み JSON を出力する'
                $'json-minify\t1 行 JSON を出力する'
                $'plain\tkey=value 形式で出力する'
            )
            _chatwork_add_described "${formats[@]}"
            return 0
            ;;
        --output|--out-dir)
            _files
            return 0
            ;;
        --chat-url)
            return 0
            ;;
        --room-id|--file-id|--message-id|--room|--var)
            return 0
            ;;
    esac

    if [[ "${mode}" == "get" && ( "${get_subcmd}" == "me" || "${get_subcmd}" == "status" || "${get_subcmd}" == "contacts" ) ]]; then
        local -a opts
        opts=(
            $'--format\t出力形式を指定する'
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "get" && "${get_subcmd}" == "room" ]]; then
        local -a opts
        opts=(
            $'--room-id\t対象ルーム ID を指定する'
            $'--chat-url\tChatwork ルーム URL を指定する'
            $'--format\t出力形式を指定する'
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "get" && "${get_subcmd}" == "message" ]]; then
        local -a opts
        opts=(
            $'--room-id\t対象ルーム ID を指定する'
            $'--message-id\t対象メッセージ ID を指定する'
            $'--chat-url\tChatwork メッセージ URL を指定する'
            $'--format\t出力形式を指定する'
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "get" && -z "${get_subcmd}" ]]; then
        local -a opts
        opts=(
            $'me\t自分のアカウント情報を表示する'
            $'status\t未読やタスクの件数を表示する'
            $'my-status\tstatus と同じ内容を表示する'
            $'contacts\tコンタクト一覧を表示する'
            $'room\tルーム情報を表示する'
            $'message\tメッセージ情報を表示する'
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "download" && "${download_subcmd}" == "file" ]]; then
        local -a opts
        opts=(
            $'--chat-url\tChatwork メッセージ URL から file_id を解決する'
            $'--room-id\t対象ルーム ID を指定する'
            $'--file-id\t対象ファイル ID を指定する'
            $'--output\t保存先ファイルパスまたは既存ディレクトリを指定する'
            $'--out-dir\t保存先ディレクトリを指定する'
            $'--force\t既存ファイルを上書きする'
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "download" && -z "${download_subcmd}" ]]; then
        local -a opts
        opts=(
            $'--chat-url\tChatwork メッセージ URL から file_id を解決する'
            $'--room-id\t対象ルーム ID を指定する'
            $'--file-id\t対象ファイル ID を指定する'
            $'--output\t保存先ファイルパスまたは既存ディレクトリを指定する'
            $'--out-dir\t保存先ディレクトリを指定する'
            $'--force\t既存ファイルを上書きする'
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "send" ]]; then
        local -a opts templates lines cmd template_specs
        opts=(
            $'--room\t送信先ルーム ID を指定する'
            $'--var\t差し込み変数を指定する'
            $'--self-unread\t自分を未読にする'
            $'--dry-run\t送信せず本文のみ表示する'
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        if [[ ${positional_seen} -eq 0 && "${cur}" != -* ]]; then
            cmd=(chatwork)
            if [[ -n "${config}" ]]; then
                cmd+=(--config "${config}")
            fi
            lines=("${(@f)$("${cmd[@]}" __complete_templates --describe --current "${cur}" 2>/dev/null)}")
            template_specs=("${lines[@]}")
            _chatwork_add_described_group 'chatwork-templates' $'\t-*- テンプレート -*-' "${template_specs[@]}"
            _chatwork_add_described_group 'chatwork-options' $'\t-*- オプション -*-' "${opts[@]}"
            return 0
        fi
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "template" && "${template_subcmd}" == "show" ]]; then
        local -a opts lines cmd template_specs
        opts=(
            $'--var\t差し込み変数を指定する'
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        if [[ ${positional_seen} -eq 0 && "${cur}" != -* ]]; then
            cmd=(chatwork)
            if [[ -n "${config}" ]]; then
                cmd+=(--config "${config}")
            fi
            lines=("${(@f)$("${cmd[@]}" __complete_templates --describe --current "${cur}" 2>/dev/null)}")
            template_specs=("${lines[@]}")
            _chatwork_add_described_group 'chatwork-templates' $'\t-*- テンプレート -*-' "${template_specs[@]}"
            _chatwork_add_described_group 'chatwork-options' $'\t-*- オプション -*-' "${opts[@]}"
            return 0
        fi
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "template" && "${template_subcmd}" == "list" ]]; then
        local -a opts
        opts=(
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "template" && -z "${template_subcmd}" ]]; then
        local -a opts
        opts=(
            $'list\tテンプレート一覧を表示する'
            $'show\tテンプレート本文を表示する'
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "completion" ]]; then
        local -a opts
        opts=(
            $'bash\tbash 用の補完スクリプトを生成する'
            $'zsh\tzsh 用の補完スクリプトを生成する'
            $'fish\tfish 用の補完スクリプトを生成する'
            $'elvish\telvish 用の補完スクリプトを生成する'
            $'power-shell\tPowerShell 用の補完スクリプトを生成する'
            $'--config\t設定ファイルのパスを指定する'
            $'--help\tヘルプを表示する'
        )
        _chatwork_add_described "${opts[@]}"
        return 0
    fi

    local -a opts
    opts=(
        $'get\t情報を取得する'
        $'download\tファイルをダウンロードする'
        $'template\tテンプレートを扱う'
        $'send\tテンプレートを送信する'
        $'completion\tシェル補完スクリプトを出力する'
        $'--config\t設定ファイルのパスを指定する'
        $'--help\tヘルプを表示する'
        $'--version\tバージョンを表示する'
        $'-h\tヘルプを表示する'
        $'-V\tバージョンを表示する'
    )
    _chatwork_add_described "${opts[@]}"
    return 0
}

compdef _chatwork chatwork
compdef -p _chatwork '*/chatwork'
"#;
