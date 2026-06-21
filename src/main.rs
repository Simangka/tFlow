use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "tflow", version, about = "Terminal text & markdown editor")]
pub struct Cli {
    #[arg(help = "Files or directories to open (syntax: file:line)")]
    files: Vec<String>,

    #[arg(short, long, help = "Theme name")]
    theme: Option<String>,

    #[arg(short = 'n', long, help = "Disable line numbers")]
    no_line_numbers: bool,

    #[arg(short = 'v', long, help = "Verbose logging")]
    verbose: bool,

    #[arg(long, help = "Log file path")]
    log_file: Option<PathBuf>,

    #[arg(short, long, help = "Command to execute on startup")]
    command: Option<String>,

    #[arg(short = 'W', long, help = "Set workspace root")]
    workspace: Option<PathBuf>,

    #[arg(long, help = "Open file at specific line:column")]
    position: Option<String>,

    #[arg(short = 'R', long, help = "Read-only mode")]
    readonly: bool,

    #[arg(short = 'w', long = "write", num_args = 1.., help = "Create empty files")]
    write: Vec<String>,

    #[arg(short = 'd', long = "dir", num_args = 1.., help = "Create directories")]
    dir: Vec<String>,
}

#[allow(dead_code)]
fn parse_file_position(s: &str) -> (PathBuf, Option<usize>, Option<usize>) {
    if let Some(idx) = s.rfind(':') {
        let (path_part, pos_part) = s.split_at(idx);
        let pos_str = &pos_part[1..];

        if let Ok(line) = pos_str.parse::<usize>() {
            return (PathBuf::from(path_part), Some(line), None);
        }

        if let Some(col_idx) = pos_str.rfind(':') {
            let (line_str, col_str) = pos_str.split_at(col_idx);
            let col_str = &col_str[1..];
            if let (Ok(line), Ok(col)) = (line_str.parse::<usize>(), col_str.parse::<usize>()) {
                return (PathBuf::from(path_part), Some(line), Some(col));
            }
        }
    }

    (PathBuf::from(s), None, None)
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    // Build CliOptions for runtime-only (non-persisted) CLI settings
    let cli_options = tflow::config::CliOptions {
        readonly: cli.readonly,
        verbose: cli.verbose,
        log_file: cli.log_file.clone(),
        files: cli.files.clone(),
        command: cli.command.clone(),
        position: cli.position.clone(),
        no_line_numbers: cli.no_line_numbers,
    };

    if cli_options.verbose || std::env::var("TFLOW_LOG").is_ok() {
        let log_file = cli_options.log_file.clone().unwrap_or_else(|| {
            let mut p = std::env::temp_dir();
            p.push("tflow.log");
            p
        });

        let file = std::fs::File::create(&log_file)?;
        let subscriber = tracing_subscriber::fmt()
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish();

        tracing::subscriber::set_global_default(subscriber)?;
        tracing::info!("tflow started with log file: {:?}", log_file);
    }

    let mut config = tflow::config::Config::load();

    // Only pass the fields that merge_cli_overrides actually uses (the rest are in CliOptions)
    config.merge_cli_overrides(
        cli.theme.as_deref(),
        cli.no_line_numbers,
        cli.verbose,
        cli.log_file.as_deref(),
        cli.command.as_deref(),
        cli.workspace.as_deref(),
        cli.position.as_deref(),
        cli.readonly,
        &cli.files,
    );

    let cwd = std::env::current_dir()?;

    if config.workspace.root_path.is_none() {
        config.workspace.root_path = Some(cwd.clone());
    }

    if !cli.write.is_empty() || !cli.dir.is_empty() {
        for path in &cli.write {
            let p = PathBuf::from(path);
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            if p.exists() {
                eprintln!("tflow: {} already exists", p.display());
            } else {
                std::fs::File::create(&p)?;
                println!("tflow: created file {}", p.display());
            }
        }
        for path in &cli.dir {
            let p = PathBuf::from(path);
            if p.exists() {
                if p.is_dir() {
                    eprintln!("tflow: {} already exists", p.display());
                } else {
                    eprintln!("tflow: {} already exists and is not a directory", p.display());
                }
            } else {
                std::fs::create_dir_all(&p)?;
                println!("tflow: created directory {}", p.display());
            }
        }
        return Ok(());
    }

    if let Err(e) = tflow::app::EventLoop::run(config).await {
        eprintln!("tflow error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
