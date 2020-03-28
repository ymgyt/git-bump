pub mod cli;

use semver::{SemVerError, Version};
use std::error::Error;
use std::io::{ Write, BufRead};

#[derive(Debug, PartialEq, Eq)]
pub enum Bump {
    Major,
    Minor,
    Patch,
}

pub fn parse_tags(tags: git2::string_array::StringArray) -> (Vec<Version>, Vec<SemVerError>) {
    let (versions, errs): (Vec<_>, Vec<_>) = tags
        .iter()
        .flatten()
        .map(|tag| tag.trim_start_matches("v"))
        .map(|tag| Version::parse(tag))
        .partition(Result::is_ok);

    (
        versions.into_iter().map(Result::unwrap).collect(),
        errs.into_iter().map(Result::unwrap_err).collect(),
    )
}

pub fn prompt_bump<W, R>(mut r: R,mut w: W, current: &Version) -> Result<Bump, Box<dyn Error>>
where
    W: Write,
    R: BufRead,
{
    let bump = loop {
        writeln!(w, "select bump version (current: {})", current)?;
        writeln!(w, "[1] major")?;
        writeln!(w, "[2] minor")?;
        writeln!(w, "[3] patch")?;

        let mut buff = String::new();
        r.read_line(&mut buff)?;
        let selected = buff.trim();

        match selected {
            "1" => break Bump::Major,
            "2" => break Bump::Minor,
            "3" => break Bump::Patch,
            _ => writeln!(w,"unexpect input {}", selected)?,
        }

    };
    Ok(bump)
}

pub fn create_tag(version: &Version, repo: &mut git2::Repository) -> Result<(), Box<dyn Error>> {
    Ok(())
}
