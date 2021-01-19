use clap::{crate_version, App, AppSettings, Arg};
pub fn parse_args() -> clap::ArgMatches<'static> {
    App::new("git-bump")
        .version(crate_version!())
        .about("bump git version tag")
        .global_setting(AppSettings::ColorAuto)
        .global_setting(AppSettings::ColoredHelp)
        .arg(
            Arg::with_name("repo")
                .long("repo")
                .alias("repository")
                .short("r")
                .help("git repository path")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("prefix")
                .long("prefix")
                .alias("version-prefix")
                .short("p")
                .help("version tag prefix")
                .takes_value(true)
                .default_value("v"),
        )
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .multiple(true)
                .help("logging verbose"),
        )
        .arg(
            Arg::with_name("no-push")
                .long("no-push")
                .help("do not push git tag to remote")
        )
        .get_matches()
}
