use git2::Repository;
use git_bump::Bump;
use std::error::Error;
use std::io::{self, Read as _, Write as _};
use tracing::{debug, error, info, warn};

fn run() -> Result<(), anyhow::Error> {
    git_bump::Config {
        ..Default::default()
    }
    .bump()
}

fn _run() -> Result<(), Box<dyn Error>> {
    // git_bump::check_credential()?;
    let arg = git_bump::cli::parse_args();

    let path = arg.value_of("repo").unwrap();
    let pattern = arg.value_of("pattern").unwrap();
    debug!("repository path: {} tag pattern: {}", path, pattern);

    let mut r = Repository::open(path)?;
    let (mut versions, errs) = git_bump::parse_tags(r.tag_names(Some(pattern))?);
    errs.into_iter().for_each(|e| match e {
        semver::SemVerError::ParseError(e) => warn!("malformed semantic version: {}", e),
    });
    versions.sort();

    let current = match versions.last() {
        None => {
            info!("tags not found (pattern: {})", pattern);
            return Ok(());
        }
        Some(v) => v,
    };

    let bump = if arg.is_present("interactive") {
        git_bump::prompt_bump(io::stdin().lock(), io::stdout(), &current)?
    } else {
        match (
            arg.is_present("major"),
            arg.is_present("minor"),
            arg.is_present("patch"),
        ) {
            (true, _, _) => Bump::Major,
            (_, true, _) => Bump::Minor,
            (_, _, true) => Bump::Patch,
            _ => git_bump::prompt_bump(io::stdin().lock(), io::stdout(), &current)?,
        }
    };

    let mut bumped = current.clone();
    match bump {
        Bump::Major => bumped.increment_major(),
        Bump::Minor => bumped.increment_minor(),
        Bump::Patch => bumped.increment_patch(),
    }
    println!("bump version {} -> {} [y/N]", current, bumped);

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    match input.to_ascii_lowercase().as_str().trim() {
        "y" | "yes" => (),
        "n" | "no" => {
            println!("canceled");
            return Ok(());
        }
        unexpected => return Err(unexpected.into()),
    };

    git_bump::create_tag(&bumped, &mut r)?;
    git_bump::push_tag(&bumped, &mut r)?;

    Ok(())
}

fn init_logger() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .without_time()
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting subscriber failed");
}

fn main() {
    init_logger();

    if let Err(err) = run() {
        error!("{}", err);
        std::process::exit(1);
    }
}
