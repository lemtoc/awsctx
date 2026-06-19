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

### `aws sso switch` alias (optional)

If you would rather type `aws sso switch` than `awsctx`, pass `--aws-wrapper`. It defines an `aws()` shell function that intercepts the (otherwise non-existent) `aws sso switch` subcommand and forwards everything else to the real AWS CLI:

```sh
awsctx init zsh --aws-wrapper
```

```sh
awsctx init bash --aws-wrapper
```

```fish
awsctx init fish --aws-wrapper
```

Then both forms work, just like running `awsctx` directly:

```sh
aws sso switch              # interactive selection
aws sso switch prod-admin   # switch by exact name
aws sso sw prod-admin       # `sw` is a shorter alias for `switch`
```

This is opt-in because it wraps the `aws` command itself. If an `aws` alias or shell function already exists, awsctx prints a warning and leaves it untouched. When the wrapper is defined, every other `aws` invocation is passed straight through to the AWS CLI.

#### Does wrapping `aws` slow things down or change behavior?

No. The wrapper is a thin shell function with a guard:

```sh
if alias aws >/dev/null 2>&1 || typeset -f aws >/dev/null 2>&1; then
  printf 'awsctx: not defining aws wrapper because aws is already an alias or function.\n' >&2
else
  function aws {
    if [ "$1" = "sso" ] && { [ "$2" = "switch" ] || [ "$2" = "sw" ]; }; then
      shift 2
      awsctx "$@"             # only this case is handled by awsctx
      return $?
    fi
    command aws "$@"          # everything else goes straight to the real aws
  }
fi
```

- **Existing wrappers are preserved.** If your shell already has an `aws` alias or function, awsctx does not replace it. This avoids bypassing tools such as `aws-vault` or company-specific AWS wrappers.
- **No measurable overhead.** For any command other than `sso switch`/`sw`, the function runs one or two shell builtin string comparisons (no subprocess, no disk access) and then execs the real binary via `command aws`. That cost is sub-microsecond — negligible next to the AWS CLI's own startup time.
- **Fully transparent.** Arguments (including spaces), stdin/stdout, pipes, and the exit code are all forwarded unchanged. `aws s3 ls`, `aws sso login`, `aws ... | jq`, and the like behave exactly as before.
- **Interactive shells only.** The function is defined only in shells that load your rc file. Non-interactive scripts (`#!/bin/bash`, CI) never see it and call the real `aws` directly.
- **Cosmetic differences only:** `type aws` / `which aws` will report a function. Tab completion for `aws` continues to work in normal setups.

If you would rather not wrap `aws` at all, just omit `--aws-wrapper`; awsctx then never touches the `aws` command.

## Switch Profiles

Run `awsctx` to select an SSO profile from your AWS config file:

```sh
awsctx
```

On success, `AWS_PROFILE` is updated in the current shell.

```text
Switched to prod-admin.
```

Switch directly by exact profile name:

```sh
awsctx prod-admin
```

Limit the candidates with `AWS_PROFILE_PREFIX`:

```sh
export AWS_PROFILE_PREFIX=prod-
awsctx
```

Limit the candidates to one AWS account ID:

```sh
export AWS_ACCOUNT_ID=111122223333
awsctx
```

When both `AWS_PROFILE_PREFIX` and `AWS_ACCOUNT_ID` are set, `awsctx` only shows profiles that match both filters.

Ignore `AWS_PROFILE_PREFIX` and `AWS_ACCOUNT_ID` for one command:

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

List all SSO profiles, ignoring `AWS_PROFILE_PREFIX` and `AWS_ACCOUNT_ID`:

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

Login to a profile directly:

```sh
awsctx login prod-admin
```

Login without changing `AWS_PROFILE`:

```sh
awsctx login --no-switch
```

```text
Logged in to prod-admin.
```

Direct login without changing `AWS_PROFILE`:

```sh
awsctx login --no-switch prod-admin
```

Ignore `AWS_PROFILE_PREFIX` and `AWS_ACCOUNT_ID` when choosing the login profile:

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
