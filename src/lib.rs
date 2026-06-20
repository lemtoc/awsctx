use anyhow::{Context, Result, anyhow};
use ini_core::{Item, Parser};
use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const SHELL_INTEGRATION_ENV: &str = "AWSCTX_SHELL_INTEGRATION";

const MANAGED_BLOCK_START: &str = "# >>> awsctx initialize >>>";
const MANAGED_BLOCK_END: &str = "# <<< awsctx initialize <<<";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SsoProfile {
    pub name: String,
    pub sso_account_id: String,
    pub sso_role_name: String,
    pub sso_session: String,
    pub region: Option<String>,
    pub output: Option<String>,
}

impl SsoProfile {
    pub fn searchable_text(&self) -> String {
        [
            self.name.as_str(),
            self.sso_account_id.as_str(),
            self.sso_role_name.as_str(),
            self.region.as_deref().unwrap_or_default(),
        ]
        .join(" ")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileOption {
    profile: SsoProfile,
    is_current: bool,
    row: String,
}

impl ProfileOption {
    fn new(profile: SsoProfile, current_profile: Option<&str>, widths: TableWidths) -> Self {
        let is_current = current_profile == Some(profile.name.as_str());
        let row = format_profile_row(&profile, widths);
        Self {
            profile,
            is_current,
            row,
        }
    }

    pub fn into_profile(self) -> SsoProfile {
        self.profile
    }

    pub fn name(&self) -> &str {
        &self.profile.name
    }

    pub fn matches_filter(&self, input: &str) -> bool {
        if input.is_empty() {
            return true;
        }

        self.profile
            .searchable_text()
            .to_lowercase()
            .contains(&input.to_lowercase())
    }
}

impl fmt::Display for ProfileOption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_current && env::var_os("NO_COLOR").is_none() {
            write!(formatter, "\x1b[32m{}\x1b[0m", self.row)
        } else {
            formatter.write_str(&self.row)
        }
    }
}

pub fn resolve_config_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("AWS_CONFIG_FILE") {
        return Ok(PathBuf::from(path));
    }

    let home = env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set."))?;
    Ok(PathBuf::from(home).join(".aws").join("config"))
}

pub fn read_config(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error).with_context(|| format!("Failed to read {}.", path.display())),
    }
}

pub fn parse_sso_profiles(input: &str) -> Result<Vec<SsoProfile>> {
    let sections = parse_sections(input)?;
    let mut seen_profile_names = HashSet::new();
    let mut profiles = Vec::new();

    for section in sections {
        let Some(profile_name) = profile_name_from_section(&section.name) else {
            continue;
        };

        if !seen_profile_names.insert(profile_name.to_owned()) {
            return Err(anyhow!("Duplicate profile: {profile_name}."));
        }

        let Some(sso_session) = section.property("sso_session") else {
            continue;
        };
        let Some(sso_account_id) = section.property("sso_account_id") else {
            continue;
        };
        let Some(sso_role_name) = section.property("sso_role_name") else {
            continue;
        };

        profiles.push(SsoProfile {
            name: profile_name.to_owned(),
            sso_account_id: sso_account_id.to_owned(),
            sso_role_name: sso_role_name.to_owned(),
            sso_session: sso_session.to_owned(),
            region: section.property("region").map(ToOwned::to_owned),
            output: section.property("output").map(ToOwned::to_owned),
        });
    }

    Ok(profiles)
}

pub fn filter_profiles(
    profiles: &[SsoProfile],
    prefix: Option<&str>,
    account_id: Option<&str>,
    include_all: bool,
) -> Vec<SsoProfile> {
    if include_all {
        return profiles.to_vec();
    }

    profiles
        .iter()
        .filter(|profile| match prefix {
            Some(prefix) if !prefix.is_empty() => profile.name.starts_with(prefix),
            _ => true,
        })
        .filter(|profile| match account_id {
            Some(account_id) if !account_id.is_empty() => profile.sso_account_id == account_id,
            _ => true,
        })
        .cloned()
        .collect()
}

pub fn find_profile_by_name(profiles: &[SsoProfile], name: &str) -> Option<SsoProfile> {
    profiles
        .iter()
        .find(|profile| profile.name == name)
        .cloned()
}

pub fn current_profile() -> Option<String> {
    env::var("AWS_PROFILE")
        .ok()
        .filter(|value| !value.is_empty())
}

pub fn profile_options(
    profiles: &[SsoProfile],
    current_profile: Option<&str>,
) -> Vec<ProfileOption> {
    let widths = table_widths(profiles);
    profiles
        .iter()
        .cloned()
        .map(|profile| ProfileOption::new(profile, current_profile, widths))
        .collect()
}

pub fn format_table(profiles: &[SsoProfile]) -> String {
    format_table_with_style(profiles, None, false)
}

pub fn format_table_with_style(
    profiles: &[SsoProfile],
    current_profile: Option<&str>,
    use_color: bool,
) -> String {
    let headers = ["NAME", "ACCOUNT_ID", "ROLE", "REGION"];
    let widths = table_widths(profiles);
    let underline = [
        "─".repeat(widths.name),
        "─".repeat(widths.account_id),
        "─".repeat(widths.role),
        "─".repeat(widths.region),
    ];

    let header = format!(
        "{:<name_width$}  {:<account_width$}  {:<role_width$}  {}",
        headers[0],
        headers[1],
        headers[2],
        headers[3],
        name_width = widths.name,
        account_width = widths.account_id,
        role_width = widths.role,
    );
    let separator = format!(
        "{:<name_width$}  {:<account_width$}  {:<role_width$}  {}",
        underline[0],
        underline[1],
        underline[2],
        underline[3],
        name_width = widths.name,
        account_width = widths.account_id,
        role_width = widths.role,
    );

    let mut lines = if use_color {
        vec![muted(&header), muted(&separator)]
    } else {
        vec![header, separator]
    };

    lines.extend(profiles.iter().map(|profile| {
        if use_color {
            format_profile_row_with_color(profile, current_profile, widths)
        } else {
            format_profile_row(profile, widths)
        }
    }));

    lines.join("\n")
}

pub fn format_profiles_json(profiles: &[SsoProfile]) -> Result<String> {
    serde_json::to_string_pretty(profiles).context("Failed to serialize profiles as JSON.")
}

pub fn activation_script(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash | Shell::Zsh => {
            r#"awsctx() {
  local __awsctx_should_switch=0
  if [ "$#" -eq 0 ] || { [ "$1" = "--all" ] && [ "$#" -eq 1 ]; }; then
    __awsctx_should_switch=1
  elif [ "$#" -eq 1 ] && [ "${1#-}" = "$1" ]; then
    case "$1" in
      list|login|activate|init|help|__switch|__login) ;;
      *) __awsctx_should_switch=1 ;;
    esac
  fi

  if [ $__awsctx_should_switch -eq 1 ]; then
    local __awsctx_profile
    __awsctx_profile="$(AWSCTX_SHELL_INTEGRATION=1 command awsctx __switch "$@")"
    local __awsctx_status=$?
    if [ $__awsctx_status -ne 0 ]; then
      return $__awsctx_status
    fi
    export AWS_PROFILE="$__awsctx_profile"
    printf 'Switched to %s.\n' "$__awsctx_profile" >&2
    return 0
  fi

  if [ "$1" = "login" ]; then
    shift
    local __awsctx_arg
    for __awsctx_arg in "$@"; do
      case "$__awsctx_arg" in
        --no-switch|-h|--help)
          command awsctx login "$@"
          return $?
          ;;
      esac
    done

    local __awsctx_profile
    __awsctx_profile="$(AWSCTX_SHELL_INTEGRATION=1 command awsctx __login "$@")"
    local __awsctx_status=$?
    if [ $__awsctx_status -ne 0 ]; then
      return $__awsctx_status
    fi
    if ! command -v aws >/dev/null 2>&1; then
      printf 'aws command not found. Install AWS CLI to use awsctx login.\n' >&2
      return 1
    fi
    aws sso login --profile "$__awsctx_profile"
    __awsctx_status=$?
    if [ $__awsctx_status -ne 0 ]; then
      return $__awsctx_status
    fi
    export AWS_PROFILE="$__awsctx_profile"
    printf 'Logged in and switched to %s.\n' "$__awsctx_profile" >&2
    return 0
  fi

  command awsctx "$@"
}
"#
        }
        Shell::Fish => {
            r#"function awsctx
  set -l __awsctx_should_switch 0
  if test (count $argv) -eq 0; or begin; test "$argv[1]" = "--all"; and test (count $argv) -eq 1; end
    set __awsctx_should_switch 1
  else if test (count $argv) -eq 1; and not string match -q -- '-*' "$argv[1]"; and not contains -- "$argv[1]" list login activate init help __switch __login
    set __awsctx_should_switch 1
  end

  if test $__awsctx_should_switch -eq 1
    set -l __awsctx_profile
    set -l __awsctx_status
    begin
      set -lx AWSCTX_SHELL_INTEGRATION 1
      set __awsctx_profile (command awsctx __switch $argv)
      set __awsctx_status $status
    end
    if test $__awsctx_status -ne 0
      return $__awsctx_status
    end
    set -gx AWS_PROFILE "$__awsctx_profile"
    printf 'Switched to %s.\n' "$__awsctx_profile" >&2
    return 0
  end

  if test "$argv[1]" = "login"
    set -l __awsctx_args $argv[2..-1]
    for __awsctx_arg in $__awsctx_args
      switch "$__awsctx_arg"
        case --no-switch -h --help
          command awsctx login $__awsctx_args
          return $status
      end
    end

    set -l __awsctx_profile
    set -l __awsctx_status
    begin
      set -lx AWSCTX_SHELL_INTEGRATION 1
      set __awsctx_profile (command awsctx __login $__awsctx_args)
      set __awsctx_status $status
    end
    if test $__awsctx_status -ne 0
      return $__awsctx_status
    end
    if not command -q aws
      printf 'aws command not found. Install AWS CLI to use awsctx login.\n' >&2
      return 1
    end
    command aws sso login --profile "$__awsctx_profile"
    set __awsctx_status $status
    if test $__awsctx_status -ne 0
      return $__awsctx_status
    end
    set -gx AWS_PROFILE "$__awsctx_profile"
    printf 'Logged in and switched to %s.\n' "$__awsctx_profile" >&2
    return 0
  end

  command awsctx $argv
end
"#
        }
    }
}

pub fn aws_wrapper_script(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash | Shell::Zsh => {
            r#"if [ "${__awsctx_aws_wrapper_loaded:-0}" != "1" ]; then
  if alias aws >/dev/null 2>&1 || typeset -f aws >/dev/null 2>&1; then
    printf 'awsctx: not defining aws wrapper because aws is already an alias or function.\n' >&2
  else
    function aws {
  if [ "$1" = "sso" ] && { [ "$2" = "switch" ] || [ "$2" = "sw" ]; }; then
    shift 2
    awsctx "$@"
    return $?
  fi

  command aws "$@"
}
    __awsctx_aws_wrapper_loaded=1
  fi
fi
"#
        }
        Shell::Fish => {
            r#"if not set -q __awsctx_aws_wrapper_loaded
  if functions -q aws
    printf 'awsctx: not defining aws wrapper because aws is already a function.\n' >&2
  else
    function aws
  if test (count $argv) -ge 2; and test "$argv[1]" = "sso"; and contains -- "$argv[2]" switch sw
    set -l __awsctx_args $argv[3..-1]
    awsctx $__awsctx_args
    return $status
  end

  command aws $argv
end
    set -g __awsctx_aws_wrapper_loaded 1
  end
end
"#
        }
    }
}

pub fn init_line(shell: Shell, aws_wrapper: bool) -> String {
    let flag = if aws_wrapper { " --aws-wrapper" } else { "" };
    match shell {
        Shell::Bash | Shell::Zsh => {
            format!("eval \"$(awsctx activate {}{flag})\"", shell.as_str())
        }
        Shell::Fish => format!("awsctx activate fish{flag} | source"),
    }
}

pub fn init_block(shell: Shell, aws_wrapper: bool) -> String {
    format!(
        "{MANAGED_BLOCK_START}\n{}\n{MANAGED_BLOCK_END}\n",
        init_line(shell, aws_wrapper)
    )
}

pub fn update_managed_block(contents: &str, block: &str) -> Result<String> {
    let Some(start) = contents.find(MANAGED_BLOCK_START) else {
        return Ok(append_block(contents, block));
    };

    let Some(end_marker_start) = contents.find(MANAGED_BLOCK_END) else {
        return Err(anyhow!(
            "Existing awsctx managed block is missing its end marker."
        ));
    };

    if end_marker_start < start {
        return Err(anyhow!(
            "Existing awsctx managed block markers are out of order."
        ));
    }

    let end = end_marker_start + MANAGED_BLOCK_END.len();
    let mut updated = String::new();
    updated.push_str(&contents[..start]);
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(block);
    if end < contents.len() {
        if !updated.ends_with('\n') {
            updated.push('\n');
        }
        updated.push_str(contents[end..].trim_start_matches('\n'));
    }
    Ok(updated)
}

pub fn init_rc_file(path: &Path, shell: Shell, aws_wrapper: bool) -> Result<()> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error).with_context(|| format!("Failed to read {}.", path.display()));
        }
    };
    let updated = update_managed_block(&contents, &init_block(shell, aws_wrapper))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}.", parent.display()))?;
    }
    fs::write(path, updated).with_context(|| format!("Failed to write {}.", path.display()))
}

pub fn rc_path(shell: Shell) -> Result<PathBuf> {
    let home = env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set."))?;
    let path = match shell {
        Shell::Bash => PathBuf::from(".bashrc"),
        Shell::Zsh => PathBuf::from(".zshrc"),
        Shell::Fish => PathBuf::from(".config").join("fish").join("config.fish"),
    };
    Ok(PathBuf::from(home).join(path))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Shell {
    Bash,
    Fish,
    Zsh,
}

impl Shell {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Fish => "fish",
            Self::Zsh => "zsh",
        }
    }
}

impl std::str::FromStr for Shell {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "bash" => Ok(Self::Bash),
            "fish" => Ok(Self::Fish),
            "zsh" => Ok(Self::Zsh),
            _ => Err(anyhow!(
                "Unsupported shell: {value}. Supported shells: bash, fish, zsh."
            )),
        }
    }
}

impl fmt::Display for Shell {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug)]
struct Section {
    name: String,
    properties: Vec<(String, String)>,
}

impl Section {
    fn property(&self, key: &str) -> Option<&str> {
        self.properties
            .iter()
            .rev()
            .find_map(|(property_key, value)| (property_key == key).then_some(value.as_str()))
    }
}

fn parse_sections(input: &str) -> Result<Vec<Section>> {
    let mut sections = Vec::new();
    let mut current_section: Option<Section> = None;

    for item in Parser::new(input).auto_trim(true) {
        match item {
            Item::Section(name) => {
                current_section = Some(Section {
                    name: name.to_owned(),
                    properties: Vec::new(),
                });
            }
            Item::Property(key, Some(value)) if !key.starts_with('#') => {
                if let Some(section) = current_section.as_mut() {
                    section.properties.push((key.to_owned(), value.to_owned()));
                }
            }
            Item::Error(error) => return Err(anyhow!("Invalid AWS config line: {error}.")),
            Item::SectionEnd => {
                if let Some(section) = current_section.take() {
                    sections.push(section);
                }
            }
            Item::Blank | Item::Comment(_) | Item::Property(_, _) => {}
        }
    }

    Ok(sections)
}

fn profile_name_from_section(section_name: &str) -> Option<&str> {
    section_name
        .strip_prefix("profile ")
        .filter(|name| !name.is_empty())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TableWidths {
    name: usize,
    account_id: usize,
    role: usize,
    region: usize,
}

fn table_widths(profiles: &[SsoProfile]) -> TableWidths {
    TableWidths {
        name: column_width(profiles, "NAME".len(), |profile| profile.name.as_str()),
        account_id: column_width(profiles, "ACCOUNT_ID".len(), |profile| {
            profile.sso_account_id.as_str()
        }),
        role: column_width(profiles, "ROLE".len(), |profile| {
            profile.sso_role_name.as_str()
        }),
        region: column_width(profiles, "REGION".len(), |profile| {
            profile.region.as_deref().unwrap_or_default()
        }),
    }
}

fn format_profile_row(profile: &SsoProfile, widths: TableWidths) -> String {
    format!(
        "{:<name_width$}  {:<account_width$}  {:<role_width$}  {}",
        profile.name,
        profile.sso_account_id,
        profile.sso_role_name,
        profile.region.as_deref().unwrap_or_default(),
        name_width = widths.name,
        account_width = widths.account_id,
        role_width = widths.role,
    )
}

fn format_profile_row_with_color(
    profile: &SsoProfile,
    current_profile: Option<&str>,
    widths: TableWidths,
) -> String {
    let name = format!("{:<width$}", profile.name, width = widths.name);
    let account_id = format!(
        "{:<width$}",
        profile.sso_account_id,
        width = widths.account_id
    );
    let role = format!("{:<width$}", profile.sso_role_name, width = widths.role);
    let region = profile.region.as_deref().unwrap_or_default();
    let name = if current_profile == Some(profile.name.as_str()) {
        green(&name)
    } else {
        cyan(&name)
    };

    format!(
        "{}  {}  {}  {}",
        name,
        secondary(&account_id),
        role,
        secondary(region)
    )
}

fn muted(value: &str) -> String {
    ansi("38;5;245", value)
}

fn secondary(value: &str) -> String {
    ansi("38;5;250", value)
}

fn green(value: &str) -> String {
    ansi("32", value)
}

fn cyan(value: &str) -> String {
    ansi("36", value)
}

fn ansi(code: &str, value: &str) -> String {
    format!("\x1b[{code}m{value}\x1b[0m")
}

fn column_width(
    profiles: &[SsoProfile],
    header_width: usize,
    value: fn(&SsoProfile) -> &str,
) -> usize {
    profiles
        .iter()
        .map(value)
        .map(str::len)
        .max()
        .unwrap_or(header_width)
        .max(header_width)
}

fn append_block(contents: &str, block: &str) -> String {
    if contents.is_empty() {
        return block.to_owned();
    }

    let separator = if contents.ends_with('\n') { "" } else { "\n" };
    format!("{contents}{separator}{block}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CONFIG: &str = r#"
[profile prod-admin]
sso_session = sso
sso_account_id = 111122223333
sso_role_name = AWSAdministratorAccess
region = ap-northeast-1
output = json

[profile dev-admin]
sso_session = sso
sso_account_id = 444455556666
sso_role_name = AWSAdministratorAccess
region = ap-northeast-1
output = json

[sso-session sso]
sso_start_url = https://example.awsapps.com/start
sso_region = ap-northeast-1

[profile derived-prod]
credential_process = aws configure export-credentials --profile prod-admin
region = ap-northeast-1
"#;

    #[test]
    fn parses_sso_profiles_in_file_order() {
        let profiles = parse_sso_profiles(SAMPLE_CONFIG).expect("profiles should parse");

        assert_eq!(
            profiles
                .iter()
                .map(|profile| profile.name.as_str())
                .collect::<Vec<_>>(),
            vec!["prod-admin", "dev-admin"]
        );
        assert_eq!(profiles[0].sso_session, "sso");
        assert_eq!(profiles[0].sso_account_id, "111122223333");
        assert_eq!(profiles[0].sso_role_name, "AWSAdministratorAccess");
        assert_eq!(profiles[0].region.as_deref(), Some("ap-northeast-1"));
        assert_eq!(profiles[0].output.as_deref(), Some("json"));
    }

    #[test]
    fn ignores_profiles_without_required_sso_fields() {
        let profiles = parse_sso_profiles(
            r#"
[profile missing-role]
sso_session = sso
sso_account_id = 123456789012

[profile valid]
sso_session = sso
sso_account_id = 123456789012
sso_role_name = Admin
"#,
        )
        .expect("profiles should parse");

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "valid");
    }

    #[test]
    fn rejects_duplicate_profile_names() {
        let error = parse_sso_profiles(
            r#"
[profile dup]
sso_session = sso
sso_account_id = 123456789012
sso_role_name = Admin

[profile dup]
sso_session = sso
sso_account_id = 210987654321
sso_role_name = Admin
"#,
        )
        .expect_err("duplicate profiles should fail");

        assert_eq!(error.to_string(), "Duplicate profile: dup.");
    }

    #[test]
    fn filters_profiles_by_case_sensitive_prefix() {
        let profiles = parse_sso_profiles(SAMPLE_CONFIG).expect("profiles should parse");

        assert_eq!(
            filter_profiles(&profiles, Some("prod"), None, false).len(),
            1
        );
        assert_eq!(
            filter_profiles(&profiles, Some("PROD"), None, false).len(),
            0
        );
        assert_eq!(filter_profiles(&profiles, Some(""), None, false).len(), 2);
        assert_eq!(
            filter_profiles(&profiles, Some("missing"), None, true).len(),
            2
        );
    }

    #[test]
    fn filters_profiles_by_account_id() {
        let profiles = parse_sso_profiles(SAMPLE_CONFIG).expect("profiles should parse");

        assert_eq!(
            filter_profiles(&profiles, None, Some("444455556666"), false)
                .first()
                .map(|profile| profile.name.as_str()),
            Some("dev-admin")
        );
        assert_eq!(
            filter_profiles(&profiles, None, Some("4444"), false).len(),
            0
        );
        assert_eq!(
            filter_profiles(&profiles, None, Some("444455556666"), true).len(),
            2
        );
    }

    #[test]
    fn combines_prefix_and_account_id_filters() {
        let profiles = parse_sso_profiles(SAMPLE_CONFIG).expect("profiles should parse");

        assert_eq!(
            filter_profiles(&profiles, Some("dev"), Some("444455556666"), false).len(),
            1
        );
        assert_eq!(
            filter_profiles(&profiles, Some("prod"), Some("444455556666"), false).len(),
            0
        );
    }

    #[test]
    fn finds_profile_by_exact_name() {
        let profiles = parse_sso_profiles(SAMPLE_CONFIG).expect("profiles should parse");

        assert_eq!(
            find_profile_by_name(&profiles, "prod-admin").map(|profile| profile.name),
            Some("prod-admin".to_owned())
        );
        assert_eq!(find_profile_by_name(&profiles, "prod"), None);
    }

    #[test]
    fn serializes_json_with_null_optional_fields() {
        let profiles = parse_sso_profiles(
            r#"
[profile valid]
sso_session = sso
sso_account_id = 123456789012
sso_role_name = Admin
"#,
        )
        .expect("profiles should parse");

        let json = format_profiles_json(&profiles).expect("json should format");

        assert!(json.contains(r#""region": null"#));
        assert!(json.contains(r#""output": null"#));
    }

    #[test]
    fn formats_table_with_header_separator() {
        let profiles = parse_sso_profiles(SAMPLE_CONFIG).expect("profiles should parse");

        let table = format_table(&profiles);
        let lines = table.lines().collect::<Vec<_>>();

        assert_eq!(
            lines[0],
            "NAME        ACCOUNT_ID    ROLE                    REGION"
        );
        assert_eq!(
            lines[1],
            "──────────  ────────────  ──────────────────────  ──────────────"
        );
        assert_eq!(
            field_start(lines[2], "111122223333"),
            field_start(lines[3], "444455556666")
        );
        assert_eq!(
            field_start(lines[2], "AWSAdministratorAccess"),
            field_start(lines[3], "AWSAdministratorAccess")
        );
        assert_eq!(
            field_start(lines[2], "ap-northeast-1"),
            field_start(lines[3], "ap-northeast-1")
        );
    }

    #[test]
    fn formats_table_with_ansi_colors() {
        let profiles = parse_sso_profiles(SAMPLE_CONFIG).expect("profiles should parse");

        let table = format_table_with_style(&profiles, Some("dev-admin"), true);
        let plain_table = strip_ansi(&table);

        assert!(table.contains("\x1b[38;5;245mNAME"));
        assert!(table.contains("\x1b[36mprod-admin"));
        assert!(table.contains("\x1b[32mdev-admin"));
        assert!(table.contains("\x1b[38;5;250m111122223333"));
        assert!(plain_table.contains("prod-admin  111122223333"));
    }

    #[test]
    fn aligns_profile_options_like_table_rows() {
        let profiles = parse_sso_profiles(SAMPLE_CONFIG).expect("profiles should parse");
        let rows = profile_options(&profiles, Some("dev-admin"))
            .iter()
            .map(ToString::to_string)
            .map(|row| strip_ansi(&row))
            .collect::<Vec<_>>();

        assert_eq!(
            field_start(&rows[0], "111122223333"),
            field_start(&rows[1], "444455556666")
        );
        assert_eq!(
            field_start(&rows[0], "AWSAdministratorAccess"),
            field_start(&rows[1], "AWSAdministratorAccess")
        );
        assert_eq!(
            field_start(&rows[0], "ap-northeast-1"),
            field_start(&rows[1], "ap-northeast-1")
        );
    }

    #[test]
    fn updates_existing_managed_block() {
        let contents =
            "before\n# >>> awsctx initialize >>>\nold\n# <<< awsctx initialize <<<\nafter\n";
        let block = init_block(Shell::Zsh, false);

        let updated = update_managed_block(contents, &block).expect("block should update");

        assert_eq!(
            updated,
            "before\n# >>> awsctx initialize >>>\neval \"$(awsctx activate zsh)\"\n# <<< awsctx initialize <<<\nafter\n"
        );
    }

    #[test]
    fn formats_fish_init_line() {
        assert_eq!(
            init_line(Shell::Fish, false),
            "awsctx activate fish | source"
        );
    }

    #[test]
    fn formats_init_line_with_aws_wrapper_flag() {
        assert_eq!(
            init_line(Shell::Zsh, true),
            "eval \"$(awsctx activate zsh --aws-wrapper)\""
        );
        assert_eq!(
            init_line(Shell::Fish, true),
            "awsctx activate fish --aws-wrapper | source"
        );
    }

    #[test]
    fn renders_posix_aws_wrapper_script() {
        let script = aws_wrapper_script(Shell::Zsh);

        assert!(script.contains("alias aws >/dev/null 2>&1"));
        assert!(script.contains("typeset -f aws >/dev/null 2>&1"));
        assert!(script.contains("not defining aws wrapper"));
        assert!(script.contains("function aws {"));
        assert!(script.contains(
            "[ \"$1\" = \"sso\" ] && { [ \"$2\" = \"switch\" ] || [ \"$2\" = \"sw\" ]; }"
        ));
        assert!(script.contains("awsctx \"$@\""));
        assert!(script.contains("command aws \"$@\""));
    }

    #[test]
    fn renders_fish_aws_wrapper_script() {
        let script = aws_wrapper_script(Shell::Fish);

        assert!(script.contains("functions -q aws"));
        assert!(script.contains("not defining aws wrapper"));
        assert!(script.contains("function aws"));
        assert!(script.contains("test (count $argv) -ge 2"));
        assert!(script.contains("test \"$argv[1]\" = \"sso\""));
        assert!(script.contains("contains -- \"$argv[2]\" switch sw"));
        assert!(script.contains("awsctx $__awsctx_args"));
        assert!(script.contains("command aws $argv"));
    }

    #[test]
    fn renders_fish_activation_script() {
        let script = activation_script(Shell::Fish);

        assert!(script.contains("function awsctx"));
        assert!(script.contains("set -l __awsctx_should_switch"));
        assert!(script.contains("set -gx AWS_PROFILE"));
        assert!(script.contains("command awsctx __switch"));
        assert!(script.contains("command awsctx __login"));
    }

    fn strip_ansi(value: &str) -> String {
        let mut stripped = String::new();
        let mut chars = value.chars().peekable();

        while let Some(character) = chars.next() {
            if character == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();
                for escape_character in chars.by_ref() {
                    if escape_character == 'm' {
                        break;
                    }
                }
            } else {
                stripped.push(character);
            }
        }

        stripped
    }

    fn field_start(line: &str, field: &str) -> usize {
        line.find(field)
            .unwrap_or_else(|| panic!("expected {field} in {line}"))
    }
}
