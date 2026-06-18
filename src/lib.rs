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
}

impl ProfileOption {
    pub fn new(profile: SsoProfile, current_profile: Option<&str>) -> Self {
        let is_current = current_profile == Some(profile.name.as_str());
        Self {
            profile,
            is_current,
        }
    }

    pub fn into_profile(self) -> SsoProfile {
        self.profile
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
        let row = format_profile_row(&self.profile);
        if self.is_current && env::var_os("NO_COLOR").is_none() {
            write!(formatter, "\x1b[32m{row}\x1b[0m")
        } else {
            formatter.write_str(&row)
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
    include_all: bool,
) -> Vec<SsoProfile> {
    match prefix {
        Some(prefix) if !include_all && !prefix.is_empty() => profiles
            .iter()
            .filter(|profile| profile.name.starts_with(prefix))
            .cloned()
            .collect(),
        _ => profiles.to_vec(),
    }
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
    profiles
        .iter()
        .cloned()
        .map(|profile| ProfileOption::new(profile, current_profile))
        .collect()
}

pub fn current_profile_index(profiles: &[SsoProfile], current_profile: Option<&str>) -> usize {
    let Some(current_profile) = current_profile else {
        return 0;
    };

    profiles
        .iter()
        .position(|profile| profile.name == current_profile)
        .unwrap_or(0)
}

pub fn format_table(profiles: &[SsoProfile]) -> String {
    let headers = ["NAME", "ACCOUNT_ID", "ROLE", "REGION"];
    let widths = [
        column_width(profiles, headers[0].len(), |profile| profile.name.as_str()),
        column_width(profiles, headers[1].len(), |profile| {
            profile.sso_account_id.as_str()
        }),
        column_width(profiles, headers[2].len(), |profile| {
            profile.sso_role_name.as_str()
        }),
        column_width(profiles, headers[3].len(), |profile| {
            profile.region.as_deref().unwrap_or_default()
        }),
    ];

    let mut lines = vec![format!(
        "{:<name_width$}  {:<account_width$}  {:<role_width$}  {:<region_width$}",
        headers[0],
        headers[1],
        headers[2],
        headers[3],
        name_width = widths[0],
        account_width = widths[1],
        role_width = widths[2],
        region_width = widths[3],
    )];

    lines.extend(profiles.iter().map(|profile| {
        format!(
            "{:<name_width$}  {:<account_width$}  {:<role_width$}  {:<region_width$}",
            profile.name,
            profile.sso_account_id,
            profile.sso_role_name,
            profile.region.as_deref().unwrap_or_default(),
            name_width = widths[0],
            account_width = widths[1],
            role_width = widths[2],
            region_width = widths[3],
        )
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
  if [ "$#" -eq 0 ] || { [ "$1" = "--all" ] && [ "$#" -eq 1 ]; }; then
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
    }
}

pub fn init_line(shell: Shell) -> String {
    format!("eval \"$(awsctx activate {})\"", shell.as_str())
}

pub fn init_block(shell: Shell) -> String {
    format!(
        "{MANAGED_BLOCK_START}\n{}\n{MANAGED_BLOCK_END}\n",
        init_line(shell)
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

pub fn init_rc_file(path: &Path, shell: Shell) -> Result<()> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error).with_context(|| format!("Failed to read {}.", path.display()));
        }
    };
    let updated = update_managed_block(&contents, &init_block(shell))?;
    fs::write(path, updated).with_context(|| format!("Failed to write {}.", path.display()))
}

pub fn rc_path(shell: Shell) -> Result<PathBuf> {
    let home = env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set."))?;
    let file_name = match shell {
        Shell::Bash => ".bashrc",
        Shell::Zsh => ".zshrc",
    };
    Ok(PathBuf::from(home).join(file_name))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Shell {
    Bash,
    Zsh,
}

impl Shell {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
        }
    }
}

impl std::str::FromStr for Shell {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "bash" => Ok(Self::Bash),
            "zsh" => Ok(Self::Zsh),
            _ => Err(anyhow!(
                "Unsupported shell: {value}. Supported shells: bash, zsh."
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

fn format_profile_row(profile: &SsoProfile) -> String {
    format!(
        "{}  {}  {}  {}",
        profile.name,
        profile.sso_account_id,
        profile.sso_role_name,
        profile.region.as_deref().unwrap_or_default()
    )
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

        assert_eq!(filter_profiles(&profiles, Some("prod"), false).len(), 1);
        assert_eq!(filter_profiles(&profiles, Some("PROD"), false).len(), 0);
        assert_eq!(filter_profiles(&profiles, Some(""), false).len(), 2);
        assert_eq!(filter_profiles(&profiles, Some("missing"), true).len(), 2);
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
    fn updates_existing_managed_block() {
        let contents =
            "before\n# >>> awsctx initialize >>>\nold\n# <<< awsctx initialize <<<\nafter\n";
        let block = init_block(Shell::Zsh);

        let updated = update_managed_block(contents, &block).expect("block should update");

        assert_eq!(
            updated,
            "before\n# >>> awsctx initialize >>>\neval \"$(awsctx activate zsh)\"\n# <<< awsctx initialize <<<\nafter\n"
        );
    }
}
