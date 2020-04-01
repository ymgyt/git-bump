use tracing::error;

fn run() -> Result<(), anyhow::Error> {
    let arg = git_bump::cli::parse_args();

    init_logger(arg.occurrences_of("verbose"));

    git_bump::Config {
        prefix: arg.value_of("prefix").map(ToOwned::to_owned),
        repository_path: arg.value_of("repo").map(ToOwned::to_owned),
        ..Default::default()
    }
    .bump()
}

fn init_logger(verbose: u64) {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .without_time()
        .with_max_level(match verbose {
            0 => tracing::Level::WARN,
            1 => tracing::Level::DEBUG,
            _ => tracing::Level::TRACE,
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting subscriber failed");
}

fn main() {
    if let Err(err) = run() {
        error!("{}", err);
        std::process::exit(1);
    }
}
