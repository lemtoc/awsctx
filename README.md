# awsctx

`awsctx` switches the active AWS SSO profile in your current shell.

It reads SSO profiles from the AWS CLI config file, provides a searchable list, and updates `AWS_PROFILE` for your current shell session.

![awsctx demo](public/hero.gif)

`awsctx` expects your AWS CLI config file (typically `~/.aws/config`) to contain AWS SSO profiles. These profiles should include fields such as `sso_session`, `sso_account_id`, and `sso_role_name`. See [public/tapes/demo-aws-config](public/tapes/demo-aws-config) for a reference example.

To test `awsctx` without modifying your actual configuration, you can point `AWS_CONFIG_FILE` to a separate file:

```sh
export AWS_CONFIG_FILE="$PWD/public/tapes/demo-aws-config"
```

## Installation

Prebuilt binaries are available for macOS and Linux. Native Windows support is not yet available; please use WSL or build from source if you wish to experiment on Windows.

### Using the shell installer (macOS/Linux)
```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/lemtoc/awsctx/releases/download/v0.2.0/awsctx-installer.sh | sh
```

### Using Homebrew
```sh
brew install lemtoc/tap/awsctx
```

### Using mise
```sh
mise use -g github:lemtoc/awsctx
```

### From source (Cargo)
```sh
cargo install --git https://github.com/lemtoc/awsctx
```

## Shell Integration

Shell integration is required for `awsctx` to modify the `AWS_PROFILE` variable in your current session.

Run the appropriate command for your shell to set it up automatically:

- **zsh**: `awsctx init zsh`
- **bash**: `awsctx init bash`
- **fish**: `awsctx init fish`

If you prefer to manage your shell configuration manually, add the following to your startup script:

**zsh / bash**:
```sh
eval "$(awsctx activate zsh)"
```

```sh
eval "$(awsctx activate bash)"
```

**fish**:
```fish
awsctx activate fish | source
```

### The `aws sso switch` alias (optional)

If you prefer typing `aws sso switch` over `awsctx`, use the `--aws-wrapper` flag. This defines an `aws()` shell function that intercepts the `aws sso switch` command while forwarding all other calls to the real AWS CLI:

```sh
awsctx init zsh --aws-wrapper
awsctx init bash --aws-wrapper
```

```fish
awsctx init fish --aws-wrapper
```

With the wrapper enabled, you can use the following:

```sh
aws sso switch              # interactive selection
aws sso switch prod-admin   # switch by exact name
aws sso sw prod-admin       # `sw` is a shorter alias for `switch`
```

This is opt-in because it wraps the `aws` command. If an `aws` alias or function already exists, `awsctx` will print a warning and leave it untouched.

#### Does wrapping `aws` impact performance or behavior?

No. The wrapper is a lightweight shell function:

- **Preserves existing wrappers**: It won't override existing `aws` aliases or functions (like those from `aws-vault`).
- **Negligible overhead**: For commands other than `sso switch`/`sw`, it runs a few shell builtin checks and immediately executes the real AWS CLI.
- **Transparent forwarding**: Arguments, stdin/stdout, pipes, and exit codes are forwarded unchanged.
- **Interactive only**: It only loads in interactive sessions and won't affect non-interactive scripts or CI/CD pipelines.

## Usage

### Switch Profiles
Run `awsctx` to select a profile interactively:
```sh
awsctx
```

On success, `AWS_PROFILE` is updated in the current shell:

```text
Switched to prod-admin.
```

Switch to a specific profile:
```sh
awsctx prod-admin
```

### Filtering Candidates
You can narrow down the list using environment variables:
```sh
export AWS_PROFILE_PREFIX=prod-
awsctx
```

```sh
export AWS_ACCOUNT_ID=111122223333
awsctx
```

When both filters are set, `awsctx` only shows profiles that match both.

To bypass filters, use the `--all` flag:
```sh
awsctx --all
```

### Listing Profiles
List matching profiles without changing your current environment:
```sh
awsctx list
```

```text
NAME        ACCOUNT_ID    ROLE                    REGION
──────────  ────────────  ──────────────────────  ──────────────
prod-admin  111122223333  AWSAdministratorAccess  ap-northeast-1
dev-admin   444455556666  AWSAdministratorAccess  ap-northeast-1
```

List all profiles, ignoring filters:

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

### SSO Login
Authenticate and then switch:
```sh
awsctx login
```

Login to a specific profile:

```sh
awsctx login prod-admin
```

To login without switching the active profile:
```sh
awsctx login --no-switch
```

```sh
awsctx login --no-switch prod-admin
```

Ignore filters when choosing the login profile:

```sh
awsctx login --all
```

## Supported Profiles

`awsctx` specifically targets AWS CLI profiles managed via **IAM Identity Center**. A profile is recognized if it contains:
- `sso_session`
- `sso_account_id`
- `sso_role_name`

> [!note]
> `[sso-session ...]` sections and profiles using `credential_process` are currently ignored.
