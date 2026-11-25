use crate::{
    project::{self, Project, ProjectName},
    search::Match,
};
use berm_lang::{
    Error, Result,
    serde_json::{Value, json},
    tool,
};
use cydonia_artifact::{
    error,
    kind::Kind,
    project::Result as ArtifactResult,
    reference::{self, Reference, Target, Within},
};

/// Create a project
///
/// Its name is unique within the calling account and is permanent.
#[tool(
    access = "accounts",
    catalog = "artifacts.project-create",
    budget(cpu = 100000, read = 1000, written = 100, memory = 16)
)]
pub fn project_create(name: String) -> Result<ProjectName> {
    Project::authenticate()?;
    Project::create(name).map_err(failure)
}

/// List projects
///
/// Return the calling account's projects in name order.
#[tool(
    query,
    access = "accounts",
    catalog = "artifacts.project-list",
    budget(cpu = 100000, read = 10000, written = 1, memory = 16)
)]
pub fn project_list() -> Result<Vec<ProjectName>> {
    Project::authenticate()?;
    Project::list().map_err(failure)
}

/// Search a project
///
/// Find up to twenty ranked article and card matches with an FTS5 query.
#[tool(
    query,
    access = "accounts",
    catalog = "artifacts.search",
    budget(cpu = 100000, read = 20000, written = 1, memory = 32)
)]
pub fn search(project: String, query: String) -> Result<Vec<Match>> {
    Project::authenticate()?;
    Project::open(&project)
        .and_then(|project| project.search(&query))
        .map_err(failure)
}

/// Read an article, board or card
///
/// Use an entry number, an article range like 12:5-7, a heading like 12#setup, or a card handle like DEV-12.
#[tool(
    query,
    access = "accounts",
    catalog = "artifacts.read",
    budget(cpu = 100000, read = 1000, written = 1, memory = 32)
)]
pub fn read(project: String, entry: String) -> Result<Value> {
    Project::authenticate()?;
    (|| {
        let project = Project::open(&project)?;
        match project.target(&entry)? {
            Target::Entry { number, within } => {
                if let Some(article) = project.read_article_entry(number, within, &entry)? {
                    return Ok(article);
                }
                let board = project
                    .board_by_number(number)?
                    .ok_or_else(|| project::missing("entry", &entry))?;
                reference::fits(&entry, Kind::Board, within)?;
                Ok(json!({
                    "kind": Kind::Board,
                    "number": number,
                    "reference": Reference::entry(None, number, None).to_string(),
                    "description": board.description,
                    "created": board.created,
                    "touched": board.touched,
                    "board": board,
                }))
            }
            Target::Card { key, handle } => project.read_card(key, handle, &entry),
        }
    })()
    .map_err(failure)
}

/// Write an article
///
/// Create a numbered article from Markdown and return its reference.
#[tool(
    access = "accounts",
    catalog = "artifacts.article-write",
    budget(cpu = 100000, read = 1000, written = 100, memory = 32)
)]
pub fn article_write(project: String, title: String, markdown: String) -> Result<Value> {
    Project::authenticate()?;
    Project::open(&project)
        .and_then(|project| project.write(title, &markdown))
        .map_err(failure)
}

/// Edit an article
///
/// Replace the Markdown named by the entry. A line range or heading replaces only that part. Include line breaks in the replacement where they should remain.
#[tool(
    access = "accounts",
    catalog = "artifacts.article-edit",
    budget(cpu = 100000, read = 1000, written = 100, memory = 32)
)]
pub fn article_edit(project: String, entry: String, markdown: String) -> Result<Value> {
    Project::authenticate()?;
    (|| {
        let project = Project::open(&project)?;
        let (number, within) = article_target(&project, &entry)?;
        project.edit(number, within, &entry, &markdown)
    })()
    .map_err(failure)
}

/// Comment on a passage
///
/// Attach a note to a line range, heading or exact quote. Use a range or heading to narrow a quote that occurs more than once.
#[tool(
    access = "accounts",
    catalog = "artifacts.comment",
    budget(cpu = 100000, read = 1000, written = 100, memory = 32)
)]
pub fn comment(
    project: String,
    entry: String,
    quote: Option<String>,
    note: String,
) -> Result<Value> {
    Project::authenticate()?;
    (|| {
        let project = Project::open(&project)?;
        let (number, within) = article_target(&project, &entry)?;
        project.add_comment(number, within, &entry, quote.as_deref(), note)
    })()
    .map_err(failure)
}

/// Add a card
///
/// Add Markdown to the end of a column. The board key and column are created on first use.
#[tool(
    access = "accounts",
    catalog = "artifacts.card-add",
    budget(cpu = 100000, read = 1000, written = 100, memory = 32)
)]
pub fn card_add(project: String, board: String, column: String, text: String) -> Result<Value> {
    Project::authenticate()?;
    Project::open(&project)
        .and_then(|project| project.add_card(&board, &column, text))
        .map_err(failure)
}

/// Move a card
///
/// Move the card to the end of a column on its board. A new column is created on first use.
#[tool(
    access = "accounts",
    catalog = "artifacts.card-move",
    budget(cpu = 100000, read = 1000, written = 100, memory = 32)
)]
pub fn card_move(project: String, card: String, column: String) -> Result<Value> {
    Project::authenticate()?;
    (|| {
        let project = Project::open(&project)?;
        let (key, handle) = card_target(&project, &card)?;
        project.move_card(key, handle, &card, &column)
    })()
    .map_err(failure)
}

/// Set a card's blockers
///
/// Replace the card's blockers. Use a handle or another project within this account such as notes#DEV-12. An empty list clears them.
#[tool(
    access = "accounts",
    catalog = "artifacts.card-blockers",
    budget(cpu = 100000, read = 10000, written = 1000, memory = 32)
)]
pub fn card_blockers(project: String, card: String, blockers: Vec<String>) -> Result<Value> {
    Project::authenticate()?;
    (|| {
        let project = Project::open(&project)?;
        let (key, handle) = card_target(&project, &card)?;
        project.set_blockers(key, handle, &card, &blockers)
    })()
    .map_err(failure)
}

fn failure(error: error::Error) -> Error {
    error.to_string().into()
}

fn article_target<'a>(
    project: &Project,
    entry: &'a str,
) -> ArtifactResult<(u64, Option<Within<'a>>)> {
    match project.target(entry)? {
        Target::Entry { number, within } => Ok((number, within)),
        Target::Card { .. } => Err(project::missing("article", entry)),
    }
}

fn card_target<'a>(project: &Project, card: &'a str) -> ArtifactResult<(&'a str, u64)> {
    match project.target(card)? {
        Target::Card { key, handle } => Ok((key, handle)),
        Target::Entry { .. } => Err(project::missing("card", card)),
    }
}
