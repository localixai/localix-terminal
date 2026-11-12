//! A repository of agent skills: the folders under `skills/` of one GitHub repository, each
//! read from its `SKILL.md`. The newest commit of its default branch is asked by its ETag, so
//! an unchanged repository costs one answer of nothing; a new commit is fetched as the
//! repository's archive, once, and every skill's frontmatter read from it.

use std::path::Path;

use serde_json::Value;

use super::{Batch, Fetch, Fetched, Source, frontmatter, origin, readable, short, text_at};
use crate::catalog::archive;
use crate::catalog::model::{Listing, Recipe, What};

/// The folder of a repository its skills are in.
const FOLDER: &str = "skills";

/// A repository of agent skills: every `skills/*/SKILL.md`, and every one a group deeper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillsRepo {
    /// Its name among sources, in the ids of its listings.
    pub id: &'static str,
    pub title: &'static str,
    /// `owner/repo` on GitHub.
    pub repo: &'static str,
    pub trust: u8,
}

impl SkillsRepo {
    /// GitHub's answer of the newest commit of the default branch.
    pub fn url(&self) -> String {
        format!(
            "https://api.github.com/repos/{}/commits?per_page=1",
            self.repo
        )
    }

    /// The skills of the repository unpacked at `folder`, taken at `commit`, as its listings:
    /// one for each folder of `skills/` with a `SKILL.md`, in the order of their names.
    pub(super) fn listings(&self, folder: &Path, commit: &str) -> Result<Vec<Listing>, String> {
        let skills = folder.join(FOLDER);
        // A skill is a folder of `skills/`, or of a group one level under it.
        let mut names = Vec::new();
        for name in folders(&skills).map_err(|why| format!("{}: {why}", skills.display()))? {
            if skills.join(&name).join("SKILL.md").is_file() {
                names.push(name);
                continue;
            }
            let inside = folders(&skills.join(&name)).unwrap_or_default();
            names.extend(inside.into_iter().map(|skill| format!("{name}/{skill}")));
        }
        names.sort();
        let owner = self.repo.split('/').next().unwrap_or(self.repo);
        let listings = names.into_iter().filter_map(|folder_name| {
            let text = std::fs::read_to_string(skills.join(&folder_name).join("SKILL.md")).ok()?;
            let keys = frontmatter::read(&text);
            let said = |key: &str| keys.get(key).cloned().filter(|value| !value.is_empty());
            let last = folder_name.rsplit('/').next().unwrap_or(&folder_name);
            let name = said("name").unwrap_or_else(|| last.to_string());
            let path = format!("{FOLDER}/{folder_name}");
            Some(Listing {
                id: format!("{}/{name}", self.id),
                what: What::Skill,
                source: self.id.to_string(),
                title: readable(&name),
                summary: said("description").unwrap_or_default(),
                author: said("author").unwrap_or_else(|| owner.to_string()),
                category: said("category").unwrap_or_default(),
                homepage: format!("https://github.com/{}/tree/HEAD/{path}", self.repo),
                version: short(commit),
                origin: origin::of_skill(self.repo, &path),
                recipe: Recipe::Skill {
                    repo: self.repo.into(),
                    rev: commit.to_string(),
                    path,
                },
                name,
            })
        });
        Ok(listings.collect())
    }
}

/// The names of the folders of `folder`.
fn folders(folder: &Path) -> std::io::Result<Vec<String>> {
    Ok(std::fs::read_dir(folder)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect())
}

impl Source for SkillsRepo {
    fn id(&self) -> &'static str {
        self.id
    }

    fn title(&self) -> &'static str {
        self.title
    }

    fn trust(&self) -> u8 {
        self.trust
    }

    fn read(
        &self,
        net: &dyn Fetch,
        mark: Option<&str>,
        cache: &Path,
    ) -> Result<Option<Batch>, String> {
        let etag = mark.filter(|mark| !mark.is_empty());
        let (text, etag) = match net.text(&self.url(), etag)? {
            Fetched::Same => return Ok(None),
            Fetched::New { text, etag } => (text, etag),
        };
        let repo = self.repo;
        let commits: Value = serde_json::from_str(&text)
            .map_err(|why| format!("GitHub's answer of {repo} is not JSON: {why}"))?;
        let commit = commits.get(0).map(|commit| text_at(commit, "sha"));
        let commit = commit.filter(|commit| commit.len() >= 12);
        let commit = commit.ok_or_else(|| format!("GitHub names no commit of {repo}"))?;
        let folder = archive::unpacked(net, cache, repo, &commit)?;
        archive::keep_only(cache, repo, &commit);
        Ok(Some(Batch {
            listings: self.listings(&folder, &commit)?,
            gone: vec![],
            mark: etag.unwrap_or_default(),
            whole: true,
            more: false,
        }))
    }
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;
