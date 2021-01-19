pub mod cli;

use anyhow::anyhow;
use colored::*;
use dialoguer::theme::ColorfulTheme;
use semver::{SemVerError, Version};
use std::borrow::Cow;
use std::io::{self, Write};
use std::result::Result as StdResult;
use tracing::{debug, warn};

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
    pub no_push: bool,
    #[doc(hidden)]
    pub __non_exhaustive: (), // https://xaeroxe.github.io/init-struct-pattern/
}

impl Default for Config {
    fn default() -> Self {
        Self {
            prefix: Some("v".to_owned()),
            repository_path: None,
            no_push: false,
            __non_exhaustive: (),
        }
    }
}

impl Config {
    pub fn bump(self) -> Result<()> {
        self.build()?.bump()
    }

    fn build(self) -> Result<Bumper> {
        let repo = match self.repository_path {
            Some(path) => git2::Repository::open(&path)?,
            None => git2::Repository::open_from_env()?,
        };
        Ok(Bumper {
            prefix: self.prefix,
            no_push: self.no_push,
            repo,
            cfg: git2::Config::open_default()?,
            w: io::stdout(),
        })
    }
}

struct Bumper {
    prefix: Option<String>,
    no_push: bool,
    repo: git2::Repository,
    cfg: git2::Config,
    w: io::Stdout,
}

impl Bumper {
    fn bump(mut self) -> Result<()> {
        let pattern = self.prefix.as_deref().map(|p| format!("{}*", p));
        let tags = self.repo.tag_names(pattern.as_deref())?;
        debug!(
            "found {} tags (pattern: {})",
            tags.len(),
            pattern.unwrap_or("".to_owned())
        );

        let (mut versions, errs) = self.parse_tags(tags);
        errs.into_iter().for_each(|e| match e {
            (tag, semver::SemVerError::ParseError(e)) => {
                warn!("malformed semantic version: {} {}", tag, e)
            }
        });
        versions.sort();

        let current = match versions.last() {
            None => {
                writeln!(
                    self.w.by_ref(),
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

        let tag_oid = self.create_tag(&bumped)?;
        debug!("create tag(object_id: {})", tag_oid);

        if self.no_push {
            return Ok(())
        }
        self.push_tag(&bumped)
    }

    fn parse_tags(
        &mut self,
        tags: git2::string_array::StringArray,
    ) -> (Vec<Version>, Vec<(String, SemVerError)>) {
        let (versions, errs): (Vec<_>, Vec<_>) = tags
            .iter()
            .flatten()
            .map(|tag| tag.trim_start_matches(self.prefix.as_deref().unwrap_or("")))
            .map(|tag| Version::parse(tag).map_err(|err| (tag.to_owned(), err)))
            .partition(StdResult::is_ok);
        (
            versions.into_iter().map(StdResult::unwrap).collect(),
            errs.into_iter().map(StdResult::unwrap_err).collect(),
        )
    }

    fn prompt_bump(&mut self, current: &Version) -> Result<Bump> {
        let selections = &["major", "minor", "patch"];
        let select = dialoguer::Select::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!("select bump version (current: {})", current))
            .default(0)
            .items(&selections[..])
            .interact()
            .unwrap();
        let bump = match select {
            0 => Bump::Major,
            1 => Bump::Minor,
            2 => Bump::Patch,
            _ => unreachable!(),
        };
        Ok(bump)
    }

    fn confirm_bump(&mut self, current: &Version, bumped: &Version) -> Result<bool> {
        let branch_name = git2::Branch::wrap(self.repo.head()?)
            .name()?
            .unwrap_or("")
            .to_owned();

        let head = self.repo.head()?.peel_to_commit()?;
        let w = self.w.by_ref();
        writeln!(w, "current HEAD")?;
        writeln!(w, "  branch : {}", branch_name)?;
        writeln!(w, "  id     : {}", head.id())?;
        writeln!(w, "  summary: {}", head.summary().unwrap_or(""))?;
        writeln!(w, "")?;
        dialoguer::Confirmation::new()
            .with_text(&format!(
                "bump version {prefix}{current} -> {prefix}{bumped}",
                prefix = format!("{}", self.prefix.as_deref().unwrap_or(""))
                    .red()
                    .bold(),
                current = format!("{}", current).red().bold(),
                bumped = format!("{}", bumped).red().bold(),
            ))
            .default(false)
            .interact()
            .map_err(anyhow::Error::from)
    }

    fn create_tag(&mut self, version: &Version) -> Result<git2::Oid> {
        let head = self.repo.head()?;
        if !head.is_branch() {
            return Err(anyhow!("HEAD is not branch"));
        }
        let obj = head.peel(git2::ObjectType::Commit)?;
        let signature = self.repo.signature()?;
        self.repo
            .tag(&format!("v{}", version), &obj, &signature, "", false)
            .map_err(anyhow::Error::from)
    }

    fn push_tag(&mut self, version: &Version) -> Result<()> {
        let mut origin = self.repo.find_remote("origin")?;

        let mut push_options = git2::PushOptions::new();
        let mut cb = git2::RemoteCallbacks::new();
        cb.transfer_progress(|_progress| {
            debug!(
                "called progress total_objects: {}",
                _progress.total_objects()
            );
            true
        })
        .push_update_reference(|reference, msg| {
            match msg {
                Some(err_msg) => println!("{}", err_msg.yellow()),
                None => println!("successfully pushed origin/{}", reference),
            }
            Ok(())
        })
        .credentials(|url, username_from_url, allowed_types| {
            debug!(
                "credential cb url:{} username_from_url:{:?} allowed_type {:?}",
                url, username_from_url, allowed_types
            );
            if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
                let user_name = match username_from_url {
                    Some(u) => Some(Cow::from(u)),
                    None => match self.user_name() {
                        Ok(Some(u)) => Some(u),
                        _ => None,
                    },
                };
                return match git2::Cred::credential_helper(&self.cfg, url, user_name.as_deref()) {
                    Ok(cred) => {
                        debug!("credential helper success");
                        Ok(cred)
                    }
                    Err(err) => {
                        debug!("{}", err);
                        // TODO: cache user credential to avoid prompt every time if user agree.
                        let cred = prompt_userpass()
                            .map_err(|_| git2::Error::from_str("prompt_userpass"))?;
                        git2::Cred::userpass_plaintext(&cred.0, &cred.1)
                    }
                };
            }
            // TODO: currently only USER_PASS_PLAINTEXT called :(
            git2::Cred::ssh_key_from_agent("xxx")
        });

        push_options.remote_callbacks(cb);

        let ref_spec = format!("refs/tags/v{0}:refs/tags/v{0}", version);
        debug!("refspec: {}", ref_spec);

        origin
            .push(&[&ref_spec], Some(&mut push_options))
            .map_err(anyhow::Error::from)
    }

    fn user_name(&self) -> Result<Option<Cow<str>>> {
        for entry in &self.cfg.entries(Some("user*"))? {
            if let Ok(entry) = entry {
                debug!("found {:?} => {:?}", entry.name(), entry.value());
                return Ok(entry.value().map(|v| Cow::Owned(String::from(v))));
            }
        }
        Ok(None)
    }
}

fn prompt_userpass() -> Result<(String, String)> {
    let username = dialoguer::Input::<String>::new()
        .with_prompt("username")
        .interact()?;
    let password = dialoguer::PasswordInput::new()
        .with_prompt("password")
        .interact()?;
    Ok((username, password))
}
