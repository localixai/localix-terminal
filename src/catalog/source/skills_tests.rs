//! Tests of `catalog::source::skills`: the repository's archive made here, its skills as the
//! real repository writes them.

use super::*;
use crate::catalog::archive::tests::archive_of;
use crate::catalog::source::tests::{Fake, new};
use crate::testing::Scratch;

const COMMIT: &str = "69c0b1a0674149f27b61b2635f935524b6add202";

const SKILLS: SkillsRepo = SkillsRepo {
    id: "skills",
    title: "Agent Skills",
    repo: "anthropics/skills",
    trust: 3,
};

/// GitHub's answer of the newest commit, cut to what is read of it.
fn commits(commit: &str) -> String {
    format!(r#"[{{"sha":"{commit}","commit":{{"message":"x"}},"parents":[]}}]"#)
}

const PDF: &str = "---\nname: pdf\ndescription: Use this skill whenever the user wants to do anything with PDF files.\nlicense: Proprietary. LICENSE.txt has complete terms\n---\n\n# PDF\n";
const DOCX: &str = "---\nname: docx\ndescription: \"Use this skill whenever the user wants to create, read, edit Word documents.\"\nauthor: Anthropic\n---\n";

#[test]
fn every_skill_of_the_repository_is_listed_from_its_frontmatter_at_the_newest_commit() {
    let scratch = Scratch::new();
    let tarball = scratch.path().join("repo.tar.gz");
    let files = [
        ("skills/pdf/SKILL.md", PDF),
        ("skills/docx/SKILL.md", DOCX),
        ("skills/no-skill/README.md", "# not one"),
        ("README.md", "# Skills"),
    ];
    archive_of(
        &scratch.path().join("made"),
        "skills-main",
        &files,
        &tarball,
    );
    let url = format!("https://codeload.github.com/anthropics/skills/tar.gz/{COMMIT}");
    let net = Fake::answering([new(&commits(COMMIT), Some("W/\"b1\"")), Ok(Fetched::Same)])
        .with_file(&url, tarball);
    let cache = scratch.path().join("cache");
    let batch = SKILLS.read(&net, None, &cache).unwrap().unwrap();
    assert_eq!(
        net.asked()[0].0,
        "https://api.github.com/repos/anthropics/skills/commits?per_page=1"
    );
    assert_eq!((batch.mark.as_str(), batch.whole), ("W/\"b1\"", true));
    let names: Vec<&str> = (batch.listings.iter())
        .map(|listing| listing.name.as_str())
        .collect();
    assert_eq!(names, ["docx", "pdf"]);
    let pdf = &batch.listings[1];
    assert_eq!(pdf.id, "skills/pdf");
    assert_eq!(pdf.what, What::Skill);
    assert_eq!(pdf.title, "Pdf");
    assert_eq!(
        pdf.summary,
        "Use this skill whenever the user wants to do anything with PDF files."
    );
    assert_eq!(pdf.version, "69c0b1a06741");
    assert_eq!(
        pdf.homepage,
        "https://github.com/anthropics/skills/tree/HEAD/skills/pdf"
    );
    assert_eq!(pdf.origin, "github.com/anthropics/skills#skills/pdf");
    // No author said: the repository's owner.
    assert_eq!(pdf.author, "anthropics");
    assert_eq!(batch.listings[0].author, "Anthropic");
    assert_eq!(
        pdf.recipe,
        Recipe::Skill {
            repo: "anthropics/skills".into(),
            rev: COMMIT.into(),
            path: "skills/pdf".into(),
        }
    );
    assert_eq!(
        batch.listings[0].summary,
        "Use this skill whenever the user wants to create, read, edit Word documents."
    );
    // The branch unchanged: nothing new, and no archive fetched again.
    assert_eq!(SKILLS.read(&net, Some(&batch.mark), &cache).unwrap(), None);
    assert_eq!(net.asked()[1].1.as_deref(), Some("W/\"b1\""));
    assert_eq!(net.fetched.lock().unwrap().len(), 1);
}

#[test]
fn an_answer_that_names_no_commit_says_why() {
    let net = Fake::answering([new(r#"{"message":"API rate limit exceeded"}"#, None)]);
    let why = SKILLS.read(&net, None, &std::env::temp_dir()).unwrap_err();
    assert!(why.contains("no commit"), "{why}");
}

#[test]
fn a_skill_kept_in_a_group_under_skills_is_listed_with_the_groups_path() {
    let scratch = Scratch::new();
    let folder = scratch.path().join("repo");
    for (path, text) in [
        ("skills/.curated/pdf/SKILL.md", PDF),
        ("skills/docx/SKILL.md", DOCX),
        ("skills/.curated/notes/README.md", "# not one"),
    ] {
        let file = folder.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    let listings = SKILLS.listings(&folder, COMMIT).unwrap();
    let paths: Vec<_> = (listings.iter())
        .map(|listing| match &listing.recipe {
            Recipe::Skill { path, .. } => (listing.name.as_str(), path.as_str()),
            Recipe::Plugin { .. } | Recipe::Server(_) => ("", ""),
        })
        .collect();
    assert_eq!(
        paths,
        [("pdf", "skills/.curated/pdf"), ("docx", "skills/docx")]
    );
}
