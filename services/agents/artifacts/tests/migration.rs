use anyhow::Result;
use rusqlite::Connection;

#[test]
fn one_article_row_indexes_the_whole_body_and_updates_in_place() -> Result<()> {
    let db = Connection::open_in_memory()?;
    db.execute_batch(include_str!("../caller/0001_artifacts.sql"))?;
    db.execute_batch(
        "INSERT INTO projects(name) VALUES ('runbooks');
         INSERT INTO articles(project, artifact, created, touched, title, markdown, properties)
         VALUES (1, 'one', 1, 1, 'Setup', 'Install wheels' || char(10) || 'Initial steps', '');",
    )?;
    let matches = |query: &str| {
        db.query_row(
            "SELECT count(*) FROM article_search WHERE article_search MATCH ?",
            [query],
            |row| row.get::<_, i64>(0),
        )
    };
    assert_eq!(matches("wheels")?, 1);
    assert_eq!(matches("Setup")?, 1);
    db.execute(
        "UPDATE articles SET markdown = ? WHERE id = 1",
        ["Now use tracks"],
    )?;
    assert_eq!(matches("wheels")?, 0);
    assert_eq!(matches("tracks")?, 1);
    assert_eq!(
        db.query_row("SELECT count(*) FROM articles", [], |row| row
            .get::<_, i64>(0))?,
        1
    );
    Ok(())
}
