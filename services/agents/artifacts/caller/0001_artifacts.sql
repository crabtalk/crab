CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
) STRICT;

CREATE TABLE entries (
    project INTEGER NOT NULL REFERENCES projects(id),
    number INTEGER NOT NULL CHECK (number > 0),
    kind TEXT NOT NULL CHECK (kind IN ('article', 'board')),
    artifact TEXT,
    PRIMARY KEY (project, number),
    UNIQUE (project, kind, artifact)
) STRICT, WITHOUT ROWID;

CREATE TABLE articles (
    id INTEGER PRIMARY KEY,
    project INTEGER NOT NULL REFERENCES projects(id),
    artifact TEXT NOT NULL,
    created INTEGER NOT NULL,
    touched INTEGER NOT NULL,
    title TEXT NOT NULL,
    markdown TEXT NOT NULL,
    properties TEXT NOT NULL,
    comments TEXT NOT NULL DEFAULT '{}',
    UNIQUE (project, artifact)
) STRICT;

CREATE VIRTUAL TABLE article_search USING fts5(title, markdown, content='articles', content_rowid='id');
CREATE TRIGGER article_insert AFTER INSERT ON articles BEGIN
    INSERT INTO article_search(rowid, title, markdown) VALUES (new.id, new.title, new.markdown);
END;
CREATE TRIGGER article_update AFTER UPDATE OF markdown, title ON articles
    WHEN old.markdown IS NOT new.markdown OR old.title IS NOT new.title BEGIN
    INSERT INTO article_search(article_search, rowid, title, markdown)
        VALUES ('delete', old.id, old.title, old.markdown);
    INSERT INTO article_search(rowid, title, markdown) VALUES (new.id, new.title, new.markdown);
END;
CREATE TRIGGER article_delete AFTER DELETE ON articles BEGIN
    INSERT INTO article_search(article_search, rowid, title, markdown)
        VALUES ('delete', old.id, old.title, old.markdown);
END;

CREATE TABLE boards (
    id INTEGER PRIMARY KEY,
    project INTEGER NOT NULL REFERENCES projects(id),
    artifact TEXT NOT NULL,
    key TEXT NOT NULL,
    created INTEGER NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    touched INTEGER NOT NULL,
    version INTEGER NOT NULL,
    document TEXT NOT NULL,
    UNIQUE (project, artifact),
    UNIQUE (project, key)
) STRICT;

CREATE TABLE card_text (
    id INTEGER PRIMARY KEY,
    board INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    handle INTEGER NOT NULL,
    markdown TEXT NOT NULL,
    UNIQUE (board, handle)
) STRICT;

CREATE VIRTUAL TABLE card_search USING fts5(markdown, content='card_text', content_rowid='id');
CREATE TRIGGER card_insert AFTER INSERT ON card_text BEGIN
    INSERT INTO card_search(rowid, markdown) VALUES (new.id, new.markdown);
END;
CREATE TRIGGER card_update AFTER UPDATE OF markdown ON card_text
    WHEN old.markdown IS NOT new.markdown BEGIN
    INSERT INTO card_search(card_search, rowid, markdown) VALUES ('delete', old.id, old.markdown);
    INSERT INTO card_search(rowid, markdown) VALUES (new.id, new.markdown);
END;
CREATE TRIGGER card_delete AFTER DELETE ON card_text BEGIN
    INSERT INTO card_search(card_search, rowid, markdown) VALUES ('delete', old.id, old.markdown);
END;
