use clap::{crate_version, App, Arg};
pub fn parse_args() -> clap::ArgMatches<'static> {
    App::new("git-bump")
        .version(crate_version!())
        .about("bump git version tag")
        .arg(
            Arg::with_name("interactive")
                .long("interactive")
                .short("i")
                .help("interactive mode"),
        )
        .arg(
            Arg::with_name("repo")
                .long("repo")
                .alias("repository")
                .short("r")
                .help("git repository(.git) path")
                .takes_value(true)
                .default_value(".git"),
        )
        .arg(
            Arg::with_name("pattern")
                .long("pattern")
                .short("p")
                .help("tag filter pattern")
                .takes_value(true)
                .default_value("v*"),
        )
        .arg(
            Arg::with_name("major")
                .long("major")
                .help("bump major version"),
        )
        .arg(
            Arg::with_name("minor")
                .long("minor")
                .help("bump minor version"),
        )
        .arg(
            Arg::with_name("patch")
                .long("patch")
                .help("bump patch version"),
        )
        .get_matches()
}
