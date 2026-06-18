# awsctx

`awsctx` switches the active AWS SSO profile in your current shell.

It reads SSO profiles from the same AWS config file used by the AWS CLI, shows a selectable list, and updates `AWS_PROFILE` for the shell session you are working in.

## Installation

Install with mise from GitHub Releases:

```sh
mise use -g github:lemtoc/awsctx
```

Install with Homebrew:

```sh
brew install lemtoc/tap/awsctx
```

Install from source with Cargo:

```sh
cargo install --git https://github.com/lemtoc/awsctx
```

After a crates.io release is available, this will also work:

```sh
cargo install awsctx
```

## Shell Integration

Shell integration is required when you want `awsctx` to change `AWS_PROFILE` in the current shell.

Add it automatically for zsh:

```sh
awsctx init zsh
```

Add it automatically for bash:

```sh
awsctx init bash
```

Add it automatically for fish:

```sh
awsctx init fish
```

If you manage your shell config yourself, add the loading line manually.

For zsh:

```sh
eval "$(awsctx activate zsh)"
```

For bash:

```sh
eval "$(awsctx activate bash)"
```

For fish:

```fish
awsctx activate fish | source
```

## Switch Profiles

Run `awsctx` to select an SSO profile from your AWS config file:

```sh
awsctx
```

On success, `AWS_PROFILE` is updated in the current shell.

```text
Switched to prod-admin.
```

Limit the candidates with `AWS_PROFILE_PREFIX`:

```sh
export AWS_PROFILE_PREFIX=prod-
awsctx
```

Ignore `AWS_PROFILE_PREFIX` for one command:

```sh
awsctx --all
```

## List Profiles

List matching SSO profiles without changing `AWS_PROFILE`:

```sh
awsctx list
```

```text
NAME        ACCOUNT_ID    ROLE                    REGION
──────────  ────────────  ──────────────────────  ──────────────
prod-admin  111122223333  AWSAdministratorAccess  ap-northeast-1
dev-admin   444455556666  AWSAdministratorAccess  ap-northeast-1
```

List all SSO profiles, ignoring `AWS_PROFILE_PREFIX`:

```sh
awsctx list --all
```

Print JSON:

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

Run `aws sso login --profile <profile>` for the selected SSO profile:

```sh
awsctx login
```

After login succeeds, `awsctx` switches to that profile.

```text
Logged in and switched to prod-admin.
```

Login without changing `AWS_PROFILE`:

```sh
awsctx login --no-switch
```

```text
Logged in to prod-admin.
```

Ignore `AWS_PROFILE_PREFIX` when choosing the login profile:

```sh
awsctx login --all
```

## Supported Profiles

`awsctx` only shows AWS CLI profiles backed by IAM Identity Center.

A profile is treated as an SSO profile when all of these fields are present:

- `sso_session`
- `sso_account_id`
- `sso_role_name`

`[sso-session ...]` sections are not profiles and are not shown. Profiles that derive credentials through settings such as `credential_process` are also ignored.
