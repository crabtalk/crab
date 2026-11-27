use berm_lang::{
    Caller, Schema, caller_storage,
    serde::{Serialize, de::DeserializeOwned},
};
use cydonia_artifact::{
    article::{Article, comments::Comments, properties::Properties},
    board::{Board, View},
    canvas::{Canvas, Document},
    error::Error,
    kind::Kind,
    project::{self, Result},
    reference::{self, Target},
    session::record::Record,
    stamp,
};

#[derive(Clone)]
pub(crate) struct Project {
    pub id: i64,
    pub name: String,
}

#[derive(Schema)]
pub struct ProjectName {
    pub name: String,
}

impl Project {
    pub fn authenticate() -> berm_lang::Result<()> {
        if !matches!(berm_lang::caller()?, Some(Caller::Outside(_))) {
            return Err(berm_lang::deny("Sign in to use Artifacts"));
        }
        let _ = stamp::set_clock(berm_lang::now);
        Ok(())
    }

    pub fn find(name: &str) -> Result<Option<Self>> {
        Ok(
            row("SELECT id FROM projects WHERE name = ?", &(name,))?.map(|(id,)| Self {
                id,
                name: name.to_owned(),
            }),
        )
    }

    pub fn open(name: &str) -> Result<Self> {
        Self::find(name)?
            .ok_or_else(|| anyhow::anyhow!("no project {name:?} in this account").into())
    }

    pub fn create(name: String) -> Result<ProjectName> {
        if !reference::segment(&name) {
            return Err(
                anyhow::anyhow!("a project name must be one nonempty reference segment").into(),
            );
        }
        if row::<(i64,)>(
            "INSERT INTO projects(name) VALUES (?) ON CONFLICT(name) DO NOTHING RETURNING id",
            &(&name,),
        )?
        .is_none()
        {
            return Err(anyhow::anyhow!("project {name:?} already exists").into());
        }
        Ok(ProjectName { name })
    }

    pub fn list() -> Result<Vec<ProjectName>> {
        Ok(rows("SELECT name FROM projects ORDER BY name", &[(); 0])?
            .into_iter()
            .map(|(name,)| ProjectName { name })
            .collect())
    }

    pub fn target<'a>(&self, entry: &'a str) -> Result<Target<'a>> {
        if let Some(target) = Target::parse(entry) {
            return Ok(target);
        }
        let parsed = reference::parse(entry)
            .filter(|reference| reference.project.is_none_or(|name| name == self.name))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "invalid entry reference {entry:?} for project {:?}",
                    self.name
                )
            })?;
        Ok(parsed.target)
    }
}

impl project::Project for Project {
    fn boards(&self) -> Vec<Board> {
        self.list_boards().unwrap_or_default()
    }

    fn board(&self, id: &str) -> Option<Board> {
        self.board_by_id(id).ok().flatten()
    }

    fn create_board(&self, name: &str, key: &str, view: View) -> Result<Board> {
        self.insert_board(name, key, view)
    }

    fn save_board(&self, board: &mut Board) -> Result<()> {
        self.update_board(board)
    }

    fn remove_board(&self, id: &str) -> Result<()> {
        if row::<(i64,)>(
            "DELETE FROM boards WHERE project = ? AND artifact = ? RETURNING id",
            &(self.id, id),
        )?
        .is_none()
        {
            return Err(missing("board", id));
        }
        self.retire(Kind::Board, id)
    }

    fn sessions(&self) -> Vec<Record> {
        Vec::new()
    }
    fn session(&self, _: &str) -> Option<Record> {
        None
    }
    fn create_session(&self) -> Result<String> {
        Err(unsupported("sessions"))
    }
    fn save_session(&self, _: &Record) -> Result<()> {
        Err(unsupported("sessions"))
    }
    fn remove_session(&self, _: &str) -> Result<()> {
        Err(unsupported("sessions"))
    }

    fn canvases(&self) -> Vec<Canvas> {
        Vec::new()
    }
    fn canvas(&self, _: &str) -> Option<Canvas> {
        None
    }
    fn create_canvas(&self, _: &str, _: &Document) -> Result<Canvas> {
        Err(unsupported("canvases"))
    }
    fn save_canvas(&self, _: &mut Canvas) -> Result<()> {
        Err(unsupported("canvases"))
    }
    fn remove_canvas(&self, _: &str) -> Result<()> {
        Err(unsupported("canvases"))
    }

    fn articles(&self) -> Vec<Article> {
        self.list_articles().unwrap_or_default()
    }

    fn article(&self, id: &str) -> Option<Article> {
        self.article_metadata(id).ok().flatten()
    }

    fn create_article(&self, markdown: &str) -> Result<Article> {
        self.insert_article(markdown, &Properties::default())
    }

    fn read_article(&self, id: &str) -> Result<String> {
        Ok(self.article_body(id)?.0)
    }

    fn write_article(&self, id: &str, markdown: &str) -> Result<()> {
        self.update_article(id, markdown)
    }

    fn comments(&self, id: &str) -> Comments {
        self.article_body(id)
            .map(|(_, comments)| comments)
            .unwrap_or_default()
    }

    fn save_comments(&self, id: &str, comments: &Comments) -> Result<()> {
        let mut comments = comments.clone();
        comments.stamp(&self.read_article(id)?);
        self.update_comments(id, &comments)
    }

    fn properties(&self, id: &str) -> Properties {
        self.article_properties(id)
            .map(|stored| cydonia_artifact::article::properties::parse(&stored))
            .unwrap_or_default()
    }

    fn save_properties(&self, id: &str, properties: &Properties) -> Result<()> {
        self.update_properties(id, properties)
    }

    fn remove_article(&self, id: &str) -> Result<()> {
        if row::<(i64,)>(
            "DELETE FROM articles WHERE project = ? AND artifact = ? RETURNING id",
            &(self.id, id),
        )?
        .is_none()
        {
            return Err(missing("article", id));
        }
        self.retire(Kind::Article, id)
    }

    fn asset(&self, _: &str, _: &str) -> Result<Vec<u8>> {
        Err(unsupported("article assets"))
    }

    fn put_asset(&self, _: &str, _: &str, _: &[u8]) -> Result<()> {
        Err(unsupported("article assets"))
    }

    fn number(&self, kind: Kind, id: &str) -> Result<u64> {
        if let Some((number,)) = row(
            "SELECT number FROM entries WHERE project = ? AND kind = ? AND artifact = ?",
            &(self.id, kind.key(), id),
        )? {
            return Ok(number);
        }
        if !matches!(kind, Kind::Article | Kind::Board) {
            return Err(unsupported(kind.key()));
        }
        let (number,) = row(
            "INSERT INTO entries(project, number, kind, artifact)
             VALUES (?1, (SELECT coalesce(max(number), 0) + 1 FROM entries WHERE project = ?1), ?2, ?3)
             RETURNING number",
            &(self.id, kind.key(), id),
        )?.ok_or_else(|| anyhow::anyhow!("entry number was not allocated"))?;
        Ok(number)
    }

    fn resolve(&self, kind: Kind, number: u64) -> Result<Option<String>> {
        let Ok(number) = i64::try_from(number) else {
            return Ok(None);
        };
        Ok(row::<(String,)>(
            "SELECT artifact FROM entries WHERE project = ? AND kind = ? AND number = ? AND artifact IS NOT NULL",
            &(self.id, kind.key(), number),
        )?.map(|(id,)| id))
    }

    fn retire(&self, kind: Kind, id: &str) -> Result<()> {
        rows::<()>(
            "UPDATE entries SET artifact = NULL WHERE project = ? AND kind = ? AND artifact = ?",
            &(self.id, kind.key(), id),
        )?;
        Ok(())
    }
}

pub(crate) fn rows<T: DeserializeOwned>(sql: &str, params: &impl Serialize) -> Result<Vec<T>> {
    caller_storage::rows(sql, params).map_err(|error| anyhow::anyhow!("{}", error.message()).into())
}

pub(crate) fn row<T: DeserializeOwned>(sql: &str, params: &impl Serialize) -> Result<Option<T>> {
    Ok(rows(sql, params)?.into_iter().next())
}

pub(crate) fn missing(what: &'static str, named: &str) -> Error {
    Error::NotFound {
        what,
        named: named.to_owned(),
    }
}

pub(crate) fn mint() -> String {
    let mut bytes = [0; 16];
    berm_lang::random(&mut bytes);
    hex::encode(bytes)
}

fn unsupported(what: &str) -> Error {
    anyhow::anyhow!("Artifacts does not store {what}").into()
}
