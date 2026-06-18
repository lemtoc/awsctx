use anyhow::{Result, anyhow};
use awsctx::{
    ProfileOption, SHELL_INTEGRATION_ENV, Shell, SsoProfile, activation_script, current_profile,
    filter_profiles, find_profile_by_name, format_profiles_json, format_table_with_style,
    init_line, init_rc_file, parse_sso_profiles, profile_options, rc_path, read_config,
    resolve_config_path,
};
use clap::{Args, Parser, Subcommand};
use inquire::{InquireError, Select};
use std::env;
use std::error::Error;
use std::fmt;
use std::io::{self, IsTerminal};
use std::process::{Command, ExitCode};

const SELECT_PAGE_SIZE: usize = 10;

#[derive(Parser, Debug)]
#[command(name = "awsctx", version, about = "Switch AWS SSO profiles")]
struct Cli {
    #[arg(
        long,
        global = true,
        help = "Ignore AWS_PROFILE_PREFIX and AWS_ACCOUNT_ID"
    )]
    all: bool,

    #[command(subcommand)]
    command: Option<CommandKind>,
}

#[derive(Subcommand, Debug)]
enum CommandKind {
    #[command(about = "List SSO profiles")]
    List(ListArgs),
    #[command(about = "Login with AWS SSO")]
    Login(LoginArgs),
    #[command(about = "Print shell integration script")]
    Activate(ShellArgs),
    #[command(about = "Add shell integration to an rc file")]
    Init(InitArgs),
    #[command(name = "__switch", hide = true)]
    SwitchInternal(ProfileNameArgs),
    #[command(name = "__login", hide = true)]
    LoginInternal(ProfileNameArgs),
    #[command(external_subcommand)]
    Profile(Vec<String>),
}

#[derive(Args, Debug)]
struct ListArgs {
    #[arg(long, help = "Output JSON")]
    json: bool,
}

#[derive(Args, Debug)]
struct LoginArgs {
    #[arg(long, help = "Login without changing AWS_PROFILE")]
    no_switch: bool,
    #[arg(value_name = "NAME")]
    profile_name: Option<String>,
}

#[derive(Args, Debug)]
struct ProfileNameArgs {
    #[arg(value_name = "NAME")]
    profile_name: Option<String>,
}

#[derive(Args, Debug)]
struct ShellArgs {
    shell: Shell,
}

#[derive(Args, Debug)]
struct InitArgs {
    #[arg(long, help = "Print the rc line without writing it")]
    print: bool,
    shell: Shell,
}

#[derive(Debug)]
struct ExitError {
    message: String,
    code: u8,
}

impl ExitError {
    fn new(message: impl Into<String>, code: u8) -> Self {
        Self {
            message: message.into(),
            code,
        }
    }
}

impl fmt::Display for ExitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ExitError {}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if let Some(exit_error) = error.downcast_ref::<ExitError>() {
                if !exit_error.message.is_empty() {
                    eprintln!("{exit_error}");
                }
                return ExitCode::from(exit_error.code);
            }

            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => switch_command(cli.all, None),
        Some(CommandKind::List(args)) => list_command(cli.all, args),
        Some(CommandKind::Login(args)) => login_command(cli.all, args),
        Some(CommandKind::Activate(args)) => {
            print!("{}", activation_script(args.shell));
            Ok(())
        }
        Some(CommandKind::Init(args)) => init_command(args),
        Some(CommandKind::SwitchInternal(args)) => {
            internal_switch_command(cli.all, args.profile_name.as_deref())
        }
        Some(CommandKind::LoginInternal(args)) => {
            internal_login_command(cli.all, args.profile_name.as_deref())
        }
        Some(CommandKind::Profile(args)) => profile_command(cli.all, &args),
    }
}

fn switch_command(include_all: bool, profile_name: Option<&str>) -> Result<()> {
    require_shell_integration()?;
    let profile = resolve_profile(include_all, profile_name)?;
    println!("{}", profile.name);
    Ok(())
}

fn internal_switch_command(include_all: bool, profile_name: Option<&str>) -> Result<()> {
    require_shell_integration()?;
    let profile = resolve_profile(include_all, profile_name)?;
    println!("{}", profile.name);
    Ok(())
}

fn profile_command(include_all: bool, args: &[String]) -> Result<()> {
    let Some(profile_name) = single_profile_name(args)? else {
        return Err(ExitError::new("Usage: awsctx <NAME>", 2).into());
    };

    switch_command(include_all, Some(profile_name))
}

fn list_command(include_all: bool, args: ListArgs) -> Result<()> {
    let (profiles, config_path) = load_profiles()?;
    let candidates = candidates_or_error(&profiles, &config_path, include_all)?;

    if args.json {
        println!("{}", format_profiles_json(&candidates)?);
    } else {
        let use_color = env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal();
        let current_profile = current_profile();
        println!(
            "{}",
            format_table_with_style(&candidates, current_profile.as_deref(), use_color)
        );
    }

    Ok(())
}

fn login_command(include_all: bool, args: LoginArgs) -> Result<()> {
    if !args.no_switch {
        require_shell_integration()?;
    }

    let profile = resolve_profile(include_all, args.profile_name.as_deref())?;
    run_aws_sso_login(&profile.name)?;

    if args.no_switch {
        eprintln!("Logged in to {}.", profile.name);
    } else {
        println!("{}", profile.name);
    }

    Ok(())
}

fn internal_login_command(include_all: bool, profile_name: Option<&str>) -> Result<()> {
    require_shell_integration()?;
    let profile = resolve_profile(include_all, profile_name)?;
    println!("{}", profile.name);
    Ok(())
}

fn init_command(args: InitArgs) -> Result<()> {
    if args.print {
        println!("{}", init_line(args.shell));
        return Ok(());
    }

    let path = rc_path(args.shell)?;
    init_rc_file(&path, args.shell)?;
    eprintln!("Updated {}.", path.display());
    Ok(())
}

fn select_profile(include_all: bool) -> Result<SsoProfile> {
    let (profiles, config_path) = load_profiles()?;
    let candidates = candidates_or_error(&profiles, &config_path, include_all)?;

    if let [profile] = candidates.as_slice() {
        return Ok(profile.clone());
    }

    let current_profile = current_profile();
    let options = profile_options(&candidates, current_profile.as_deref());
    let selected = Select::new("Select AWS profile", options)
        .with_page_size(SELECT_PAGE_SIZE)
        .with_scorer(&profile_scorer)
        .with_sorter(&keep_config_order)
        .prompt()
        .map_err(map_inquire_error)?;

    Ok(selected.into_profile())
}

fn resolve_profile(include_all: bool, profile_name: Option<&str>) -> Result<SsoProfile> {
    match profile_name {
        Some(name) => profile_by_name(name),
        None => select_profile(include_all),
    }
}

fn profile_by_name(profile_name: &str) -> Result<SsoProfile> {
    let (profiles, config_path) = load_profiles()?;

    if profiles.is_empty() {
        return Err(ExitError::new(
            format!("No SSO profiles found in {}.", config_path.display()),
            1,
        )
        .into());
    }

    find_profile_by_name(&profiles, profile_name).ok_or_else(|| {
        ExitError::new(
            format!(
                "No SSO profile named {} in {}.",
                profile_name,
                config_path.display()
            ),
            1,
        )
        .into()
    })
}

fn single_profile_name(args: &[String]) -> Result<Option<&str>> {
    match args {
        [] => Ok(None),
        [profile_name] => Ok(Some(profile_name.as_str())),
        _ => Err(ExitError::new("Usage: awsctx <NAME>", 2).into()),
    }
}

fn load_profiles() -> Result<(Vec<SsoProfile>, std::path::PathBuf)> {
    let config_path = resolve_config_path()?;
    let content = read_config(&config_path)?;
    let profiles = parse_sso_profiles(&content)?;
    Ok((profiles, config_path))
}

fn candidates_or_error(
    profiles: &[SsoProfile],
    config_path: &std::path::Path,
    include_all: bool,
) -> Result<Vec<SsoProfile>> {
    if profiles.is_empty() {
        return Err(ExitError::new(
            format!("No SSO profiles found in {}.", config_path.display()),
            1,
        )
        .into());
    }

    let prefix = non_empty_env("AWS_PROFILE_PREFIX");
    let account_id = non_empty_env("AWS_ACCOUNT_ID");
    let candidates = filter_profiles(
        profiles,
        prefix.as_deref(),
        account_id.as_deref(),
        include_all,
    );
    if candidates.is_empty() {
        let filters = active_filter_descriptions(prefix.as_deref(), account_id.as_deref());
        if !filters.is_empty() {
            return Err(ExitError::new(
                format!(
                    "No SSO profiles match {} in {}.",
                    filters.join(" and "),
                    config_path.display()
                ),
                1,
            )
            .into());
        }

        return Err(ExitError::new(
            format!("No SSO profiles found in {}.", config_path.display()),
            1,
        )
        .into());
    }

    Ok(candidates)
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

fn active_filter_descriptions(prefix: Option<&str>, account_id: Option<&str>) -> Vec<String> {
    let mut filters = Vec::new();

    if let Some(prefix) = prefix {
        filters.push(format!("AWS_PROFILE_PREFIX={prefix}"));
    }

    if let Some(account_id) = account_id {
        filters.push(format!("AWS_ACCOUNT_ID={account_id}"));
    }

    filters
}

fn require_shell_integration() -> Result<()> {
    if env::var_os(SHELL_INTEGRATION_ENV).is_some() {
        return Ok(());
    }

    Err(ExitError::new(
        "Shell integration is not active. Run awsctx init <shell> or load awsctx activate <shell> in your shell config.",
        1,
    )
    .into())
}

fn run_aws_sso_login(profile_name: &str) -> Result<()> {
    let status = Command::new("aws")
        .args(["sso", "login", "--profile", profile_name])
        .status()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ExitError::new(
                    "aws command not found. Install AWS CLI to use awsctx login.",
                    1,
                )
                .into()
            } else {
                anyhow!(error).context("Failed to run aws sso login.")
            }
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(ExitError::new(
            String::new(),
            status.code().unwrap_or(1).try_into().unwrap_or(1),
        )
        .into())
    }
}

fn map_inquire_error(error: InquireError) -> anyhow::Error {
    match error {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            ExitError::new("Canceled.", 130).into()
        }
        other => anyhow!(other),
    }
}

fn profile_scorer(
    input: &str,
    option: &ProfileOption,
    _string_value: &str,
    _index: usize,
) -> Option<i64> {
    option.matches_filter(input).then_some(0)
}

fn keep_config_order(_options: &mut [(usize, i64)]) {}
