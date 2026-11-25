use crate::project::{self, Project};
use berm_lang::serde_json::{self, Value, json};
use cydonia_artifact::{
    board::{
        Board, Card, View,
        blocking::{self, Found, Place, Shelf},
        column, key,
    },
    error::Error,
    kind::Kind,
    project::{Project as _, Result},
    reference::{self, Target},
};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

type StoredBoard = (String, u64, Option<u64>, u64, u64, String);

impl Project {
    pub fn list_boards(&self) -> Result<Vec<Board>> {
        project::rows(
            "SELECT b.document, b.version, e.number, b.created, b.touched, b.description FROM boards b
             LEFT JOIN entries e ON e.project = b.project AND e.kind = 'board' AND e.artifact = b.artifact
             WHERE b.project = ? ORDER BY b.touched DESC, b.id DESC",
            &(self.id,),
        )?.into_iter().map(stored).collect()
    }

    pub fn board_by_id(&self, id: &str) -> Result<Option<Board>> {
        project::row(
            "SELECT b.document, b.version, e.number, b.created, b.touched, b.description FROM boards b
             LEFT JOIN entries e ON e.project = b.project AND e.kind = 'board' AND e.artifact = b.artifact
             WHERE b.project = ? AND b.artifact = ?",
            &(self.id, id),
        )?.map(stored).transpose()
    }

    pub fn board_by_key(&self, key: &str) -> Result<Option<Board>> {
        project::row(
            "SELECT b.document, b.version, e.number, b.created, b.touched, b.description FROM boards b
             LEFT JOIN entries e ON e.project = b.project AND e.kind = 'board' AND e.artifact = b.artifact
             WHERE b.project = ? AND b.key = ?",
            &(self.id, key.to_ascii_uppercase()),
        )?.map(stored).transpose()
    }

    pub fn board_by_number(&self, number: u64) -> Result<Option<Board>> {
        match self.resolve(Kind::Board, number)? {
            Some(id) => self.board_by_id(&id),
            None => Ok(None),
        }
    }

    pub fn insert_board(&self, name: &str, written: &str, view: View) -> Result<Board> {
        let key = match key::normalize(written) {
            Some(key) => key,
            None => {
                let taken = project::rows::<(String,)>(
                    "SELECT key FROM boards WHERE project = ?",
                    &(self.id,),
                )?
                .into_iter()
                .map(|(key,)| key)
                .collect::<HashSet<_>>();
                key::derive(name, &taken)
            }
        };
        let mut board = Board::new(project::mint(), name);
        board.key = key;
        board.view = view;
        board.created = Some(board.touched);
        let version = 1;
        if project::row::<(i64,)>(
            "INSERT INTO boards(project, artifact, key, created, touched, version, document)
             VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6) ON CONFLICT(project, key) DO NOTHING RETURNING id",
            &(
                self.id,
                &board.id,
                &board.key,
                berm_lang::now(),
                version,
                serde_json::to_string(&board)?,
            ),
        )?
        .is_none()
        {
            return Err(Error::KeyTaken { key: board.key });
        }
        board.number = Some(self.number(Kind::Board, &board.id)?);
        board.version = Some(version.to_string());
        Ok(board)
    }

    pub fn update_board(&self, board: &mut Board) -> Result<()> {
        let previous = self.board_by_id(&board.id)?.ok_or(Error::Stale)?;
        if board.version.is_some() && board.version != previous.version {
            return Err(Error::Stale);
        }
        let now = berm_lang::now();
        board.touched = now.into();
        let version = previous
            .version
            .as_deref()
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or(Error::Stale)?;
        let (id, next): (i64, u64) = project::row(
            "UPDATE boards SET document = ?, key = ?, touched = ?, description = ?, version = version + 1
             WHERE project = ? AND artifact = ? AND version = ? RETURNING id, version",
            &(
                serde_json::to_string(board)?,
                &board.key,
                now,
                &board.description,
                self.id,
                &board.id,
                version,
            ),
        )?
        .ok_or(Error::Stale)?;
        let mut indexed: BTreeMap<u64, &str> = previous
            .columns
            .iter()
            .flat_map(|lane| &lane.cards)
            .filter_map(|card| Some((card.handle?, card.text.as_str())))
            .collect();
        for card in board.columns.iter().flat_map(|lane| &lane.cards) {
            let Some(handle) = card.handle else { continue };
            if indexed.remove(&handle) != Some(card.text.as_str()) {
                project::rows::<()>(
                    "INSERT INTO card_text(board, handle, markdown) VALUES (?, ?, ?)
                     ON CONFLICT(board, handle) DO UPDATE SET markdown = excluded.markdown",
                    &(id, handle, &card.text),
                )?;
            }
        }
        for handle in indexed.keys() {
            project::rows::<()>(
                "DELETE FROM card_text WHERE board = ? AND handle = ?",
                &(id, handle),
            )?;
        }
        board.version = Some(next.to_string());
        Ok(())
    }

    pub fn add_card(&self, board: &str, column: &str, text: String) -> Result<Value> {
        let key = key::normalize(board)
            .filter(|key| key.eq_ignore_ascii_case(board))
            .ok_or_else(|| {
                anyhow::anyhow!("a board key must contain only ASCII letters and digits")
            })?;
        if text.trim().is_empty() {
            return Err(anyhow::anyhow!("a card needs text").into());
        }
        let mut board = match self.board_by_key(&key)? {
            Some(board) => board,
            None => self.create_board(board, &key, View::default())?,
        };
        let lane = column_id(&mut board, column)?;
        let id = board
            .add_card(&lane, text)
            .ok_or_else(|| project::missing("column", column))?
            .id
            .clone();
        self.save_board(&mut board)?;
        card_in(&board, &id)
    }

    pub fn read_card(&self, key: &str, handle: u64, named: &str) -> Result<Value> {
        let board = self
            .board_by_key(key)?
            .ok_or_else(|| project::missing("card", named))?;
        let card = board
            .card_at(handle)
            .ok_or_else(|| project::missing("card", named))?;
        card_in(&board, &card.id)
    }

    pub fn move_card(&self, key: &str, handle: u64, named: &str, column: &str) -> Result<Value> {
        let mut board = self
            .board_by_key(key)?
            .ok_or_else(|| project::missing("card", named))?;
        let id = board
            .card_at(handle)
            .ok_or_else(|| project::missing("card", named))?
            .id
            .clone();
        let column = column_id(&mut board, column)?;
        if board.move_card(&id, &column) {
            self.save_board(&mut board)?;
        }
        card_in(&board, &id)
    }

    pub fn set_blockers(
        &self,
        key: &str,
        handle: u64,
        named: &str,
        wanted: &[String],
    ) -> Result<Value> {
        let board = self
            .board_by_key(key)?
            .ok_or_else(|| project::missing("card", named))?;
        let card = board
            .card_at(handle)
            .ok_or_else(|| project::missing("card", named))?;
        let card_id = card.id.clone();
        let board_id = board.id.clone();
        let home = PathBuf::from(&self.name);
        let waiter =
            Place::of(&home, &board, card).ok_or_else(|| project::missing("card", named))?;
        let held = card.blocked_by.clone();
        let mut shelf = Boards::default();
        shelf.keep(self, board);
        let mut normalized = Vec::new();
        for name in wanted {
            let place = shelf
                .load_card(self, name)?
                .ok_or_else(|| project::missing("card", name))?;
            let reference = place.written(&home);
            if !normalized.contains(&reference) {
                normalized.push(reference);
            }
        }
        for name in &held {
            shelf.load_card(self, name)?;
        }
        let edits =
            blocking::set_blockers(&shelf, &waiter, &board_id, &card_id, &held, &normalized)
                .map_err(|name| project::missing("card", &name))?;
        for (project, id) in blocking::boards(&edits) {
            let (project_store, boards) = shelf
                .0
                .get_mut(&project)
                .ok_or_else(|| project::missing("project", &project.to_string_lossy()))?;
            let board = boards
                .values_mut()
                .find(|board| board.id == id)
                .ok_or_else(|| project::missing("board", &id))?;
            if blocking::apply(&edits, &project, board) {
                project_store.save_board(board)?;
            }
        }
        let board = shelf
            .board(&home, key)
            .ok_or_else(|| project::missing("board", key))?;
        card_in(board, &card_id)
    }
}

fn stored(
    (document, version, number, created, touched, description): StoredBoard,
) -> Result<Board> {
    let mut board: Board = serde_json::from_str(&document)?;
    board.created = Some(created.into());
    board.touched = touched.into();
    board.description = description;
    board.version = Some(version.to_string());
    board.number = number;
    Ok(board)
}

fn card_in(board: &Board, id: &str) -> Result<Value> {
    let card = board.card(id).ok_or_else(|| project::missing("card", id))?;
    let column = board
        .column_of(id)
        .ok_or_else(|| project::missing("card", id))?;
    Ok(json!({
        "kind": "card",
        "reference": board.handle_of(card),
        "board": board.number,
        "column": column.name,
        "card": card,
    }))
}

fn column_id(board: &mut Board, name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow::anyhow!("a column needs a name").into());
    }
    let heading = column::heading(name);
    if let Some(column) = board
        .columns
        .iter()
        .find(|column| column.id == name || column.name == heading)
    {
        return Ok(column.id.clone());
    }
    Ok(board.add_column(name).id.clone())
}

#[derive(Default)]
struct Boards(BTreeMap<PathBuf, (Project, BTreeMap<String, Board>)>);

impl Boards {
    fn keep(&mut self, project: &Project, board: Board) {
        self.0
            .entry(PathBuf::from(&project.name))
            .or_insert_with(|| (project.clone(), BTreeMap::new()))
            .1
            .insert(board.key.clone(), board);
    }

    fn board(&self, project: &Path, key: &str) -> Option<&Board> {
        self.0.get(project)?.1.get(&key.to_ascii_uppercase())
    }

    fn load_card(&mut self, home: &Project, name: &str) -> Result<Option<Place>> {
        let Some(reference) =
            reference::parse(name).filter(|reference| reference.hosted().is_none())
        else {
            return Ok(None);
        };
        let Target::Card { key, handle } = reference.target else {
            return Ok(None);
        };
        let name = reference.project.unwrap_or(&home.name);
        let path = Path::new(name);
        if self.board(path, key).is_none() {
            let project = if name == home.name {
                Some(home.clone())
            } else {
                Project::find(name)?
            };
            let Some(project) = project else {
                return Ok(None);
            };
            let Some(board) = project.board_by_key(key)? else {
                return Ok(None);
            };
            self.keep(&project, board);
        }
        Ok(self
            .board(path, key)
            .and_then(|board| Place::of(path, board, board.card_at(handle)?)))
    }
}

impl Shelf for Boards {
    fn project(&self, from: &Path, name: Option<&str>) -> Option<PathBuf> {
        let path = name.map_or(from, Path::new);
        self.0.contains_key(path).then(|| path.to_path_buf())
    }

    fn card(&self, project: &Path, key: &str, handle: u64) -> Option<Found> {
        let board = self.board(project, key)?;
        let card: &Card = board.card_at(handle)?;
        Some(Found {
            board: board.id.clone(),
            key: board.key.clone(),
            card: card.id.clone(),
        })
    }
}
