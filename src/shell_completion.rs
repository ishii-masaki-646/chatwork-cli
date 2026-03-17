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

const BASH_SCRIPT: &str = r#"_chatwork() {
    local cur prev
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev=""
    if (( COMP_CWORD > 0 )); then
        prev="${COMP_WORDS[COMP_CWORD-1]}"
    fi

    local config=""
    local mode=""
    local template_subcmd=""
    local positional_seen=0
    local i word

    for ((i=1; i<COMP_CWORD; i++)); do
        word="${COMP_WORDS[i]}"
        case "${word}" in
            --config)
                if (( i + 1 < COMP_CWORD )); then
                    ((i++))
                    config="${COMP_WORDS[i]}"
                fi
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
        --room|--var)
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

    if [[ "${mode}" == "template" && -z "${template_subcmd}" ]]; then
        COMPREPLY=( $(compgen -W "list show --config --help" -- "${cur}") )
        return 0
    fi

    if [[ "${mode}" == "completion" ]]; then
        COMPREPLY=( $(compgen -W "bash zsh fish elvish power-shell --config --help" -- "${cur}") )
        return 0
    fi

    COMPREPLY=( $(compgen -W "template send completion --config --help --version -h -V" -- "${cur}") )
    return 0
}

complete -F _chatwork chatwork
"#;

const ZSH_SCRIPT: &str = r#"#compdef chatwork

_chatwork() {
    local cur prev
    cur="${words[CURRENT]}"
    prev=""
    if (( CURRENT > 1 )); then
        prev="${words[CURRENT-1]}"
    fi

    local config=""
    local mode=""
    local template_subcmd=""
    local positional_seen=0
    local i word

    for ((i=2; i<CURRENT; i++)); do
        word="${words[i]}"
        case "${word}" in
            --config)
                if (( i + 1 < CURRENT )); then
                    ((i++))
                    config="${words[i]}"
                fi
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
        --room|--var)
            return 0
            ;;
    esac

    if [[ "${mode}" == "send" ]]; then
        local -a opts templates descriptions display_strings lines cmd
        opts=(--room --var --self-unread --dry-run --config --help)
        if [[ ${positional_seen} -eq 0 && "${cur}" != -* ]]; then
            cmd=(chatwork)
            if [[ -n "${config}" ]]; then
                cmd+=(--config "${config}")
            fi
            lines=("${(@f)$("${cmd[@]}" __complete_templates --describe --current "${cur}" 2>/dev/null)}")
            templates=()
            descriptions=()
            display_strings=()
            local line name description
            for line in "${lines[@]}"; do
                name="${line%%$'\t'*}"
                description="${line#*$'\t'}"
                templates+=("${name}")
                descriptions+=("${description}")
                if [[ -n "${description}" ]]; then
                    display_strings+=("${name} -- ${description}")
                else
                    display_strings+=("${name}")
                fi
            done
            (( ${#templates[@]} > 0 )) && compadd -l -o match -d display_strings -- "${templates[@]}"
            compadd -- "${opts[@]}"
            return 0
        fi
        compadd -- "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "template" && "${template_subcmd}" == "show" ]]; then
        local -a opts templates descriptions display_strings lines cmd
        opts=(--var --config --help)
        if [[ ${positional_seen} -eq 0 && "${cur}" != -* ]]; then
            cmd=(chatwork)
            if [[ -n "${config}" ]]; then
                cmd+=(--config "${config}")
            fi
            lines=("${(@f)$("${cmd[@]}" __complete_templates --describe --current "${cur}" 2>/dev/null)}")
            templates=()
            descriptions=()
            display_strings=()
            local line name description
            for line in "${lines[@]}"; do
                name="${line%%$'\t'*}"
                description="${line#*$'\t'}"
                templates+=("${name}")
                descriptions+=("${description}")
                if [[ -n "${description}" ]]; then
                    display_strings+=("${name} -- ${description}")
                else
                    display_strings+=("${name}")
                fi
            done
            (( ${#templates[@]} > 0 )) && compadd -l -o match -d display_strings -- "${templates[@]}"
            compadd -- "${opts[@]}"
            return 0
        fi
        compadd -- "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "template" && -z "${template_subcmd}" ]]; then
        local -a opts
        opts=(list show --config --help)
        compadd -- "${opts[@]}"
        return 0
    fi

    if [[ "${mode}" == "completion" ]]; then
        local -a opts
        opts=(bash zsh fish elvish power-shell --config --help)
        compadd -- "${opts[@]}"
        return 0
    fi

    local -a opts
    opts=(template send completion --config --help --version -h -V)
    compadd -- "${opts[@]}"
    return 0
}

compdef _chatwork chatwork
compdef -p _chatwork '*/chatwork'
"#;
