pub mod cli;

use colored::*;
use semver::{SemVerError, Version};
use std::error::Error;
use std::io::{self, BufRead, Read, Write};
use std::ops::Add;
use std::result::Result as StdResult;
#[allow(unused)]
use tracing::{debug, error, warn};

#[derive(Debug, PartialEq, Eq)]
pub enum Bump {
    Major,
    Minor,
    Patch,
}

type Result<T> = std::result::Result<T, anyhow::Error>;

pub struct Config {
    pub prefix: Option<String>,
    pub repository_path: Option<String>,
    #[doc(hidden)]
    pub __non_exhaustive: (), // https://xaeroxe.github.io/init-struct-pattern/
}

impl Default for Config {
    fn default() -> Self {
        Self {
            prefix: Some("v".to_owned()),
            repository_path: None,
            __non_exhaustive: (),
        }
    }
}

impl Config {
    pub fn bump(self) -> Result<()> {
        self.build()?.bump()
    }

    fn build(self) -> Result<Bumper<io::Stdin, io::Stdout>> {
        let repo = match self.repository_path {
            Some(path) => git2::Repository::open(&path)?,
            None => git2::Repository::open_from_env()?,
        };
        Ok(Bumper {
            prefix: self.prefix,
            r: io::stdin(),
            w: io::stdout(),
            repo,
        })
    }
}

struct Bumper<R, W> {
    prefix: Option<String>,
    r: R,
    w: W,
    repo: git2::Repository,
}

impl<R, W> Bumper<R, W>
where
    R: io::Read,
    W: io::Write,
{
    fn bump(mut self) -> Result<()> {
        let pattern = if self.prefix.is_some() {
            Some(format!("{}*", self.prefix.as_ref().unwrap()))
        } else {
            None
        };
        let tags = self.repo.tag_names(pattern.as_deref())?;
        debug!(
            "found {} tags (pattern: {})",
            tags.len(),
            self.prefix.as_deref().unwrap_or("")
        );

        let (mut versions, errs) = self.parse_tags(tags);
        errs.into_iter().for_each(|e| match e {
            semver::SemVerError::ParseError(e) => warn!("malformed semantic version: {}", e),
        });
        versions.sort();

        let current = match versions.last() {
            None => {
                writeln!(
                    &mut self.w,
                    "{} (pattern: {})",
                    "version tag not found".red(),
                    self.prefix.as_deref().unwrap_or("")
                )?;
                return Ok(());
            }
            Some(v) => v,
        };

        let mut bumped = current.clone();
        match self.prompt_bump(&current)? {
            Bump::Major => bumped.increment_major(),
            Bump::Minor => bumped.increment_minor(),
            Bump::Patch => bumped.increment_patch(),
        }

        if !self.confirm_bump(&current, &bumped)? {
            writeln!(self.w.by_ref(), "canceled")?;
            return Ok(());
        }

        Ok(())
    }
    fn parse_tags(
        &mut self,
        tags: git2::string_array::StringArray,
    ) -> (Vec<Version>, Vec<SemVerError>) {
        let (versions, errs): (Vec<_>, Vec<_>) = tags
            .iter()
            .flatten()
            .map(|tag| tag.trim_start_matches(self.prefix.as_deref().unwrap_or("")))
            .map(|tag| Version::parse(tag))
            .partition(StdResult::is_ok);
        (
            versions.into_iter().map(StdResult::unwrap).collect(),
            errs.into_iter().map(StdResult::unwrap_err).collect(),
        )
    }

    fn prompt_bump(&mut self, current: &Version) -> Result<Bump> {
        let bump = loop {
            writeln!(&mut self.w, "select bump version (current: {})", current)?;
            writeln!(&mut self.w, "[1] major")?;
            writeln!(&mut self.w, "[2] minor")?;
            writeln!(&mut self.w, "[3] patch")?;

            // let input = self.r.bytes().next().and_then(|r|r.ok());
            match self.read_char() {
                '1' => break Bump::Major,
                '2' => break Bump::Minor,
                '3' => break Bump::Patch,
                unexpected => writeln!(self.w, "unexpect input {}", unexpected)?,
            }
        };
        Ok(bump)
    }

    fn confirm_bump(&mut self, current: &Version, bumped: &Version) -> Result<bool> {
        let branch_name = git2::Branch::wrap(self.repo.head()?)
            .name()?
            .unwrap_or("")
            .to_owned();
        {
            // drop head borrow
            let head = self.repo.head()?.peel_to_commit()?;
            let w = self.w.by_ref();
            writeln!(w, "branch: {}", branch_name)?;
            writeln!(w, "{}", "commit:".black().bold())?;
            writeln!(w, "       id: {}", head.id())?;
            writeln!(w, "  summary: {}", head.summary().unwrap_or(""))?;
            writeln!(
                w,
                "bump version {prefix}{current} -> {prefix}{bumped} [y/N]",
                prefix = self.prefix.as_deref().unwrap_or(""),
                current = current,
                bumped = bumped,
            )?;
        }

        Ok(self.read_char().to_ascii_lowercase() == 'y')
    }

    fn read_char(&mut self) -> char {
        self.r
            .by_ref()
            .bytes()
            .next()
            .and_then(|r| r.ok())
            .map(|byte| byte as char)
            .expect("read input failed")
    }
}

pub fn parse_tags(tags: git2::string_array::StringArray) -> (Vec<Version>, Vec<SemVerError>) {
    let (versions, errs): (Vec<_>, Vec<_>) = tags
        .iter()
        .flatten()
        .map(|tag| tag.trim_start_matches("v"))
        .map(|tag| Version::parse(tag))
        .partition(StdResult::is_ok);

    (
        versions.into_iter().map(StdResult::unwrap).collect(),
        errs.into_iter().map(StdResult::unwrap_err).collect(),
    )
}

pub fn prompt_bump<W, R>(
    mut r: R,
    mut w: W,
    current: &Version,
) -> std::result::Result<Bump, Box<dyn Error>>
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
            _ => writeln!(w, "unexpect input {}", selected)?,
        }
    };
    Ok(bump)
}

pub fn create_tag(
    version: &Version,
    repo: &mut git2::Repository,
) -> std::result::Result<(), Box<dyn Error>> {
    let head = repo.head()?;
    if !head.is_branch() {
        return Err(Box::<dyn Error>::from("HEAD is not branch!"));
    }
    let obj = head.peel(git2::ObjectType::Commit)?;

    let signature = repo.signature()?;

    let obj_id = repo.tag(&format!("v{}", version), &obj, &signature, "", false)?;
    Ok(())
}

pub fn push_tag(
    version: &Version,
    repo: &mut git2::Repository,
) -> std::result::Result<(), anyhow::Error> {
    let mut origin = repo.find_remote("origin")?;
    let cfg = git2::Config::open_default()?;

    let mut push_options = git2::PushOptions::new();
    let mut cb = git2::RemoteCallbacks::new();
    cb.transfer_progress(|_progress| {
        println!(
            "called progress total_objects: {}",
            _progress.total_objects()
        );
        true
    })
    .push_update_reference(|reference, msg| {
        println!("push_update_reference ref: {}, msg: {:?}", reference, msg);
        Ok(())
    })
    .credentials(|url, username_from_url, allowed_types| {
        println!(
            "credential cb url:{} from_url:{:?} allowed_type{:?}",
            url, username_from_url, allowed_types
        );
        if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            let r = git2::Cred::credential_helper(&cfg, url, username_from_url);
            if r.is_err() {
                eprintln!("{:?}", r.as_ref().err());
                // TODO: prompt password
                return git2::Cred::userpass_plaintext("ymgyt", "bepythonic2");
            }
            return r;
        }
        git2::Cred::ssh_key_from_agent("ymgyt")
    });

    push_options.remote_callbacks(cb);

    let ref_spec = format!("refs/tags/v{0}:refs/tags/v{0}", version);

    origin
        .push(&[&ref_spec], Some(&mut push_options))
        .map_err(anyhow::Error::from)
}

pub fn check_credential() -> std::result::Result<(), anyhow::Error> {
    let c = git2::Config::open_default().unwrap();
    for entry in &c.entries(None).unwrap() {
        let entry = entry.unwrap();
        println!("{} => {}", entry.name().unwrap(), entry.value().unwrap());
    }

    let ssh_key = git2::Cred::ssh_key(
        "ymgyt",
        None,
        std::path::Path::new(&format!("{}/.ssh/id_rsa", std::env::var("HOME").unwrap())),
        None,
    )
    .unwrap();
    println!(
        "(ssh_key) has_user_name: {}, type: {}",
        ssh_key.has_username(),
        ssh_key.credtype()
    );

    let r = git2::Cred::ssh_key_from_agent("ymgyt").unwrap();
    println!(
        "has_user_name: {}, type: {}",
        r.has_username(),
        r.credtype()
    );
    Err(anyhow::anyhow!("debug"))
}
