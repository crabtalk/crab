use crate::project::{self, Project};
use berm_lang::serde_json::{self, Value, json};
use cydonia_artifact::{
    article::{
        Article, anchor,
        comments::{Comment, Comments},
        properties::{self, Properties},
    },
    kind::Kind,
    project::{Project as _, Result},
    reference::{self, Within},
};
use std::ops::Range;

type Metadata = (String, u64, u64, String);
type StoredArticle = (String, u64, u64, String, String, String);

impl Project {
    pub fn write(&self, title: String, markdown: &str) -> Result<Value> {
        let article = self.insert_article(
            markdown,
            &Properties {
                title,
                ..Properties::default()
            },
        )?;
        let number = self.number(Kind::Article, &article.id)?;
        Ok(
            json!({ "number": number, "reference": reference::Reference::entry(None, number, None).to_string(), "article": article }),
        )
    }

    pub fn read_article_entry(
        &self,
        number: u64,
        within: Option<Within<'_>>,
        named: &str,
    ) -> Result<Option<Value>> {
        let Ok(sql_number) = i64::try_from(number) else {
            return Ok(None);
        };
        let Some((id, created, touched, stored, markdown, comments)) = project::row::<StoredArticle>(
            "SELECT a.artifact, a.created, a.touched, a.properties, a.markdown, a.comments
             FROM entries e JOIN articles a ON a.project = e.project AND a.artifact = e.artifact
             WHERE e.project = ? AND e.number = ? AND e.kind = 'article'",
            &(self.id, sql_number),
        )?
        else {
            return Ok(None);
        };
        let article = metadata((id, created, touched, stored));
        let mut comments: Comments = serde_json::from_str(&comments)?;
        let range = part(&markdown, within, named)?;
        if within.is_some() {
            comments.comments.retain(|note| {
                note.start < range.end && range.start < note.end.max(note.start.saturating_add(1))
            });
        }
        Ok(Some(json!({
            "kind": Kind::Article,
            "number": number,
            "reference": reference::Reference::entry(None, number, within).to_string(),
            "article": article,
            "markdown": &markdown[range.clone()],
            "lines": (within.is_some() && !range.is_empty()).then(|| anchor::span_of(&markdown, range).to_string()),
            "comments": comments,
        })))
    }

    pub fn edit(
        &self,
        number: u64,
        within: Option<Within<'_>>,
        named: &str,
        markdown: &str,
    ) -> Result<Value> {
        let id = self
            .resolve(Kind::Article, number)?
            .ok_or_else(|| project::missing("article", named))?;
        let (mut body, comments) = self.article_body(&id)?;
        let range = part(&body, within, named)?;
        if within.is_some() {
            let old = body.clone();
            body.replace_range(range, markdown);
            self.replace_article(&id, &old, &body, comments)?;
        } else {
            self.replace_article(&id, &body, markdown, comments)?;
        }
        Ok(json!({ "reference": reference::Reference::entry(None, number, None).to_string() }))
    }

    pub fn add_comment(
        &self,
        number: u64,
        within: Option<Within<'_>>,
        named: &str,
        quote: Option<&str>,
        note: String,
    ) -> Result<Value> {
        if note.trim().is_empty() {
            return Err(anyhow::anyhow!("a comment needs a note").into());
        }
        let id = self
            .resolve(Kind::Article, number)?
            .ok_or_else(|| project::missing("article", named))?;
        let (body, mut comments) = self.article_body(&id)?;
        let mut range = part(&body, within, named)?;
        if let Some(quote) = quote {
            if quote.is_empty() {
                return Err(anyhow::anyhow!("a comment quote cannot be empty").into());
            }
            let mut matches = body[range.clone()].match_indices(quote);
            let (offset, _) = matches
                .next()
                .ok_or_else(|| anyhow::anyhow!("the quote does not occur in {named}"))?;
            if matches.next().is_some() {
                return Err(anyhow::anyhow!("the quote occurs more than once in {named}").into());
            }
            let start = range.start + offset;
            range = start..start + quote.len();
        } else if within.is_none() {
            return Err(
                anyhow::anyhow!("give a quote, line range or heading for the comment").into(),
            );
        }
        let comment = Comment::new(
            comments.fresh_id(berm_lang::now()),
            &body,
            range,
            note,
            None,
        );
        let reply = json!({
            "reference": reference::Reference::entry(None, number, within).to_string(),
            "comment": &comment,
        });
        comments.comments.push(comment);
        comments.stamp(&body);
        self.update_comments(&id, &comments)?;
        Ok(reply)
    }

    pub fn list_articles(&self) -> Result<Vec<Article>> {
        Ok(project::rows(
            "SELECT artifact, created, touched, properties FROM articles WHERE project = ? ORDER BY touched DESC, id DESC",
            &(self.id,),
        )?.into_iter().map(metadata).collect())
    }

    pub fn article_metadata(&self, id: &str) -> Result<Option<Article>> {
        Ok(project::row(
            "SELECT artifact, created, touched, properties FROM articles WHERE project = ? AND artifact = ?",
            &(self.id, id),
        )?.map(metadata))
    }

    pub fn article_properties(&self, id: &str) -> Result<String> {
        let (stored,) = project::row(
            "SELECT properties FROM articles WHERE project = ? AND artifact = ?",
            &(self.id, id),
        )?
        .ok_or_else(|| project::missing("article", id))?;
        Ok(stored)
    }

    pub fn article_body(&self, id: &str) -> Result<(String, Comments)> {
        let (markdown, comments): (_, String) = project::row(
            "SELECT markdown, comments FROM articles WHERE project = ? AND artifact = ?",
            &(self.id, id),
        )?
        .ok_or_else(|| project::missing("article", id))?;
        Ok((markdown, serde_json::from_str(&comments)?))
    }

    pub fn insert_article(&self, markdown: &str, properties: &Properties) -> Result<Article> {
        let id = project::mint();
        let now = berm_lang::now();
        let stored = properties::apply("", properties).unwrap_or_default();
        project::rows::<()>(
            "INSERT INTO articles(project, artifact, created, touched, title, markdown, properties)
             VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6)",
            &(self.id, &id, now, &properties.title, markdown, &stored),
        )?;
        self.number(Kind::Article, &id)?;
        Ok(metadata((id, now, now, stored)))
    }

    pub fn update_article(&self, id: &str, markdown: &str) -> Result<()> {
        let (old, comments) = self.article_body(id)?;
        self.replace_article(id, &old, markdown, comments)
    }

    fn replace_article(
        &self,
        id: &str,
        old: &str,
        markdown: &str,
        mut comments: Comments,
    ) -> Result<()> {
        if old == markdown {
            return Ok(());
        }
        if !comments.comments.is_empty() {
            comments.carry(Some(old), markdown);
        }
        project::rows::<()>(
            "UPDATE articles SET markdown = ?, comments = ?, touched = ? WHERE project = ? AND artifact = ?",
            &(
                markdown,
                serde_json::to_string(&comments)?,
                berm_lang::now(),
                self.id,
                id,
            ),
        )?;
        Ok(())
    }

    pub fn update_comments(&self, id: &str, comments: &Comments) -> Result<()> {
        project::rows::<()>(
            "UPDATE articles SET comments = ? WHERE project = ? AND artifact = ?",
            &(serde_json::to_string(comments)?, self.id, id),
        )?;
        Ok(())
    }

    pub fn update_properties(&self, id: &str, properties: &Properties) -> Result<()> {
        let old = self.article_properties(id)?;
        let stored = properties::apply(&old, properties).unwrap_or_default();
        project::rows::<()>(
            "UPDATE articles SET title = ?, properties = ?, touched = ? WHERE project = ? AND artifact = ?",
            &(&properties.title, stored, berm_lang::now(), self.id, id),
        )?;
        Ok(())
    }
}

fn metadata((id, created, touched, stored): Metadata) -> Article {
    let properties = properties::parse(&stored);
    Article {
        id,
        title: properties.title,
        archived: properties.archived,
        touched: touched.into(),
        created: Some(created.into()),
        cover: None,
        labels: properties.labels,
    }
}

fn part(markdown: &str, within: Option<Within<'_>>, named: &str) -> Result<Range<usize>> {
    match within {
        Some(within) => Ok(anchor::excerpt(markdown, within, named)?.0),
        None => Ok(0..markdown.len()),
    }
}
