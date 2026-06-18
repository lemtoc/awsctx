# awsctx

awsctx は、AWS IAM Identity Center を使う SSO Profile を選び、現在のシェルの `AWS_PROFILE` を切り替えるための CLI ツールです。

## インストール

GitHub Releases からのインストールを主な導線にします。

```sh
mise use -g github:lemtoc/awsctx
```

Homebrew tap からもインストールできるようにします。

```sh
brew install lemtoc/tap/awsctx
```

Rust 環境がある場合は `cargo install` も使えるようにします。

```sh
cargo install awsctx
```

## シェル連携

`awsctx` が現在のシェルの `AWS_PROFILE` を変更するには、シェル連携が必要です。

自動で `.zshrc` に追加する場合:

```sh
awsctx init zsh
```

自動で `.bashrc` に追加する場合:

```sh
awsctx init bash
```

自動で fish の `config.fish` に追加する場合:

```sh
awsctx init fish
```

dotfiles などで手動管理したい場合は、次の 1 行を rc ファイルへ追加します。

```sh
eval "$(awsctx activate zsh)"
```

bash の場合:

```sh
eval "$(awsctx activate bash)"
```

fish の場合:

```fish
awsctx activate fish | source
```

## 切り替え

`awsctx` を実行すると、`~/.aws/config` から SSO Profile を読み取り、選択 UI を表示します。

```sh
awsctx
```

切り替えに成功すると、現在のシェルの `AWS_PROFILE` が更新されます。

```text
Switched to prod-admin.
```

`AWS_PROFILE_PREFIX` を設定すると、候補を前方一致で絞り込めます。

```sh
export AWS_PROFILE_PREFIX=prod-
awsctx
```

絞り込みを一時的に無視して全 SSO Profile から選ぶ場合:

```sh
awsctx --all
```

## 一覧表示

SSO Profile を一覧表示します。`AWS_PROFILE` は変更しません。

```sh
awsctx list
```

```text
NAME        ACCOUNT_ID    ROLE                    REGION
prod-admin  111122223333  AWSAdministratorAccess  ap-northeast-1
dev-admin   444455556666  AWSAdministratorAccess  ap-northeast-1
```

`AWS_PROFILE_PREFIX` を無視して一覧表示する場合:

```sh
awsctx list --all
```

JSON で出力する場合:

```sh
awsctx list --json
```

```json
[
  {
    "name": "prod-admin",
    "sso_account_id": "111122223333",
    "sso_role_name": "AWSAdministratorAccess",
    "sso_session": "sso",
    "region": "ap-northeast-1",
    "output": "json"
  }
]
```

## SSO Login

選択した SSO Profile に対して `aws sso login --profile <profile>` を実行します。ログインに成功すると、デフォルトではその SSO Profile に切り替えます。

```sh
awsctx login
```

```text
Logged in and switched to prod-admin.
```

ログインだけを行い、`AWS_PROFILE` を変更しない場合:

```sh
awsctx login --no-switch
```

```text
Logged in to prod-admin.
```

絞り込みを一時的に無視して全 SSO Profile からログイン先を選ぶ場合:

```sh
awsctx login --all
```

## 対象になる Profile

awsctx は SSO Profile 専用です。AWS 設定ファイルの `[profile ...]` のうち、次の 3 つがすべて設定されたものだけを対象にします。

- `sso_session`
- `sso_account_id`
- `sso_role_name`

`[sso-session ...]` 自体は切り替え対象ではありません。`credential_process` などで別の AWS Profile から資格情報を導出する Profile も対象にしません。

## リリース

`v0.1.0` のような SemVer タグを push すると、GitHub Actions が GitHub Release 用の成果物と Homebrew formula を生成します。

Homebrew tap へ formula を push するには、`lemtoc/awsctx` 側の GitHub Actions secret に `HOMEBREW_TAP_TOKEN` を設定します。この token には `lemtoc/homebrew-tap` へ push できる権限が必要です。
