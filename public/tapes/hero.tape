# awsctx hero demo: switch AWS profiles in the current shell
Source public/tapes/config.tape
Output public/hero.gif
Require nix

Hide
Type "unset NO_COLOR"
Enter
Type "export TERM=xterm-256color COLORTERM=truecolor CLICOLOR=1 CLICOLOR_FORCE=1 FORCE_COLOR=1"
Enter
Type "nix develop -c cargo build -q"
Enter
Sleep 1s
Type "export PATH=$PWD/target/debug:$PATH"
Enter
Type "export AWS_CONFIG_FILE=$PWD/public/tapes/demo-aws-config"
Enter
Type "precmd() { if [[ -n $AWS_PROFILE ]]; then PROMPT=$'\n%F{yellow}[AWS_PROFILE='$AWS_PROFILE']%f %F{green}>%f '; else PROMPT=$'\n%F{green}>%f '; fi }"
Enter
Type "source <(awsctx activate zsh)"
Enter
Type "clear"
Enter
Sleep 500ms
Show

Type "awsctx list"
Sleep 700ms
Enter
Sleep 2.5s

Type "awsctx"
Sleep 700ms
Enter
Wait+Screen /Select AWS profile:/
Sleep 700ms
Type "sand"
Sleep 2s
Enter
Wait+Screen /Switched to sandbox\/readOnly./
Sleep 1.5s

Type "export AWS_ACCOUNT_ID=111122223333"
Sleep 900ms
Enter
Sleep 1.5s

Type "awsctx"
Sleep 700ms
Enter
Wait+Screen /Select AWS profile:/
Sleep 700ms
Down 2
Sleep 1.5s
Enter
Wait+Screen /Switched to sandbox\/admin./
Sleep 4s
