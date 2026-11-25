use crate::project::{self, Project};
use berm_lang::Schema;
use cydonia_artifact::{
    project::Result,
    reference::{Reference, Span, Target, Within},
};

const LIMIT: u64 = 20;

#[derive(Schema)]
pub struct Match {
    pub reference: String,
    pub title: String,
    pub line: Option<u64>,
    pub snippet: String,
}

impl Project {
    pub fn search(&self, query: &str) -> Result<Vec<Match>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let marker = project::mint();
        let hits: Vec<(u64, String, String, Option<u64>, f64)> = project::rows(
            "WITH hits AS MATERIALIZED (
                SELECT e.number, a.title, snippet(article_search, -1, '', '', '…', 24) AS snippet,
                       highlight(article_search, 1, ?4, '') AS highlighted, rank
                FROM article_search
                JOIN articles a ON a.id = article_search.rowid
                JOIN entries e ON e.project = a.project AND e.kind = 'article' AND e.artifact = a.artifact
                WHERE article_search MATCH ?1 AND a.project = ?2
                ORDER BY rank LIMIT ?3
             ), offsets AS (
                SELECT number, title, snippet, instr(highlighted, ?4) AS position, highlighted, rank
                FROM hits
             ), prefixes AS (
                SELECT number, title, snippet, CASE WHEN position > 0 THEN
                       substr(highlighted, 1, position - 1) END AS prefix, rank
                FROM offsets
             )
             SELECT number, title, snippet, 1 + length(prefix) - length(replace(prefix, char(10), '')), rank
             FROM prefixes",
            &(query, self.id, LIMIT, marker),
        )?;
        let cards: Vec<(String, u64, String, String, f64)> = project::rows(
            "SELECT b.key, c.handle, json_extract(b.document, '$.name'),
                    snippet(card_search, 0, '', '', '…', 24), rank
             FROM card_search JOIN card_text c ON c.id = card_search.rowid JOIN boards b ON b.id = c.board
             WHERE card_search MATCH ? AND b.project = ?
             ORDER BY rank LIMIT ?",
            &(query, self.id, LIMIT),
        )?;
        let mut matches = Vec::new();
        for (number, title, snippet, line, rank) in hits {
            let within = line.map(|line| {
                Within::Span(Span {
                    from: line,
                    to: line,
                })
            });
            matches.push((
                rank,
                Match {
                    reference: Reference::entry(None, number, within).to_string(),
                    title,
                    line,
                    snippet,
                },
            ));
        }
        for (key, handle, title, snippet, rank) in cards {
            matches.push((
                rank,
                Match {
                    reference: Target::Card { key: &key, handle }.to_string(),
                    title,
                    line: None,
                    snippet,
                },
            ));
        }
        matches.sort_by(|(a, _), (b, _)| a.total_cmp(b));
        Ok(matches
            .into_iter()
            .take(LIMIT as usize)
            .map(|(_, found)| found)
            .collect())
    }
}
