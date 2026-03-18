use clap::{Parser, Subcommand, ValueEnum};
use elegy_core::{
    compose_runtime, validate_descriptor_set, Catalog, ConfigInspection, CoreError, Diagnostic,
    ProjectLocator, ResourceFamily, Severity, CLI_SCHEMA_VERSION,
};
use elegy_host_mcp::{serve_stdio, HostError};
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "elegy")]
#[command(about = "Bootstrap CLI for Elegy MCP v1")]
struct Cli {
    #[arg(long)]
    project: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
enum Command {
    Validate {
        #[command(subcommand)]
        command: ValidateCommand,
    },
    Inspect {
        #[command(subcommand)]
        command: InspectCommand,
    },
    Run {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ValidateCommand {
    Config,
    Runtime,
}

#[derive(Subcommand, Debug)]
enum InspectCommand {
    Resources,
}

#[derive(Serialize)]
struct Envelope<T>
where
    T: Serialize,
{
    schema_version: &'static str,
    command: Vec<String>,
    status: &'static str,
    summary: Summary,
    data: T,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Default, Serialize)]
struct Summary {
    errors: usize,
    warnings: usize,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("unexpected CLI failure: {error}");
            ExitCode::from(2)
        }
    }
}

async fn run() -> Result<ExitCode, serde_json::Error> {
    let cli = Cli::parse();
    let locator = cli
        .project
        .map_or(ProjectLocator::Auto, ProjectLocator::Path);

    match cli.command {
        Command::Validate {
            command: ValidateCommand::Config,
        } => execute_config_command(locator, cli.format, vec!["validate", "config"]),
        Command::Validate {
            command: ValidateCommand::Runtime,
        } => execute_runtime_command(locator, cli.format, vec!["validate", "runtime"]),
        Command::Inspect {
            command: InspectCommand::Resources,
        } => execute_runtime_command(locator, cli.format, vec!["inspect", "resources"]),
        Command::Run { dry_run } => execute_run_command(locator, dry_run, cli.format).await,
    }
}

fn execute_config_command(
    locator: ProjectLocator,
    format: OutputFormat,
    command: Vec<&str>,
) -> Result<ExitCode, serde_json::Error> {
    match validate_descriptor_set(locator) {
        Ok(inspection) => {
            let summary = Summary::default();
            match format {
                OutputFormat::Text => print_config_text(&inspection),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: command.into_iter().map(str::to_string).collect(),
                    status: "ok",
                    summary,
                    data: inspection,
                    diagnostics: Vec::new(),
                })?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(error, format, command, json!({})),
    }
}

fn execute_runtime_command(
    locator: ProjectLocator,
    format: OutputFormat,
    command: Vec<&str>,
) -> Result<ExitCode, serde_json::Error> {
    match compose_runtime(locator) {
        Ok(catalog) => {
            let summary = Summary::default();
            match format {
                OutputFormat::Text => print_catalog_text(&catalog),
                OutputFormat::Json => print_json(&Envelope {
                    schema_version: CLI_SCHEMA_VERSION,
                    command: command.into_iter().map(str::to_string).collect(),
                    status: "ok",
                    summary,
                    data: catalog,
                    diagnostics: Vec::new(),
                })?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => emit_error(error, format, command, json!({})),
    }
}

async fn execute_run_command(
    locator: ProjectLocator,
    dry_run: bool,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    if !dry_run {
        if format != OutputFormat::Text {
            let diagnostic = Diagnostic::error(
                "CLI-RUN-002",
                "live stdio host mode does not support `--format json`",
            );
            return emit_diagnostics(
                format,
                vec!["run"],
                vec![diagnostic],
                json!({}),
                "error",
                ExitCode::from(2),
            );
        }

        return match serve_stdio(locator).await {
            Ok(()) => Ok(ExitCode::SUCCESS),
            Err(HostError::Core(error)) => emit_error(error, format, vec!["run"], json!({})),
            Err(error) => emit_diagnostics(
                format,
                vec!["run"],
                vec![Diagnostic::error("CLI-RUN-003", error.to_string())],
                json!({}),
                "error",
                ExitCode::from(2),
            ),
        };
    }

    execute_runtime_command(locator, format, vec!["run", "dry-run"])
}

fn emit_error<T: Serialize>(
    error: CoreError,
    format: OutputFormat,
    command: Vec<&str>,
    data: T,
) -> Result<ExitCode, serde_json::Error> {
    emit_diagnostics(
        format,
        command,
        error.diagnostics().to_vec(),
        data,
        "invalid",
        ExitCode::from(1),
    )
}

fn emit_diagnostics<T: Serialize>(
    format: OutputFormat,
    command: Vec<&str>,
    diagnostics: Vec<Diagnostic>,
    data: T,
    status: &'static str,
    exit_code: ExitCode,
) -> Result<ExitCode, serde_json::Error> {
    let summary = summarize(&diagnostics);
    match format {
        OutputFormat::Text => print_diagnostics_text(&diagnostics),
        OutputFormat::Json => print_json(&Envelope {
            schema_version: CLI_SCHEMA_VERSION,
            command: command.into_iter().map(str::to_string).collect(),
            status,
            summary,
            data,
            diagnostics,
        })?,
    }
    Ok(exit_code)
}

fn summarize(diagnostics: &[Diagnostic]) -> Summary {
    Summary {
        errors: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count(),
        warnings: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Warning)
            .count(),
    }
}

fn print_config_text(inspection: &ConfigInspection) {
    println!("configuration is valid");
    println!("project: {}", inspection.project_name);
    println!("root config: {}", inspection.root_config);
    println!("descriptor files: {}", inspection.descriptor_files.len());
    println!("resources: {}", inspection.resource_count);
}

fn print_catalog_text(catalog: &Catalog) {
    println!("runtime is valid");
    println!("project: {}", catalog.project_name);
    println!("resources: {}", catalog.resource_count);
    for resource in &catalog.resources {
        println!(
            "- {} [{}] {}",
            resource.uri,
            format_family(resource.family),
            resource.id
        );
    }
}

fn print_diagnostics_text(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        let mut location = String::new();
        if let Some(path) = &diagnostic.location.path {
            location.push_str(path);
        }
        if let Some(field) = &diagnostic.location.field {
            if !location.is_empty() {
                location.push('#');
            }
            location.push_str(field);
        }

        if location.is_empty() {
            eprintln!(
                "{}[{}]: {}",
                severity_label(diagnostic),
                diagnostic.code,
                diagnostic.message
            );
        } else {
            eprintln!(
                "{}[{}] {}: {}",
                severity_label(diagnostic),
                diagnostic.code,
                location,
                diagnostic.message
            );
        }

        if let Some(hint) = &diagnostic.hint {
            eprintln!("  hint: {hint}");
        }
    }
}

fn severity_label(diagnostic: &Diagnostic) -> &'static str {
    match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn format_family(family: ResourceFamily) -> &'static str {
    match family {
        ResourceFamily::Static => "static",
        ResourceFamily::Filesystem => "filesystem",
        ResourceFamily::Http => "http",
        ResourceFamily::OpenApi => "open_api",
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
