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

    #[arg(short = 'w', long, help = "Set workspace root")]
    workspace: Option<PathBuf>,

    #[arg(long, help = "Open file at specific line:column")]
    position: Option<String>,

    #[arg(short = 'R', long, help = "Read-only mode")]
    readonly: bool,
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

fn install_panic_hook() {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        prev_hook(info);
    }));
}

fn init_logging(cli: &Cli) -> Result<(), anyhow::Error> {
    if cli.verbose || std::env::var("TFLOW_LOG").is_ok() {
        let log_file = cli.log_file.clone().unwrap_or_else(|| {
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
    } else {
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("tflow=info,warn"));
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    install_panic_hook();

    if let Err(e) = init_logging(&cli) {
        eprintln!("logging init failed: {}", e);
    }

    let mut config = tflow::config::Config::load();

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

    if let Err(e) = tflow::app::EventLoop::run(config).await {
        eprintln!("tflow error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
