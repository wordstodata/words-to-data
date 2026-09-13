//! SQLite storage backend for datasets

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use rusqlite::{Connection, params};

use crate::congress::{BillVotes, HouseRollCall, Member, MemberVote, SponsorInfo, VotePosition};
use crate::dataset::{
    DatasetError, DatasetMetadata, Expression, ExpressionId, ExpressionInfo, ExpressionPair,
    SearchResult, WorkId,
};
use crate::diff::TreeDiff;
use crate::document::DocumentNode;
use crate::intern::StringInterner;
use crate::link::{Link, LinkKind, Provenance, Target};
use crate::storage::memory::{ExpressionsByWork, require_same_work};
use crate::storage::{
    DocumentReader, DocumentWriter, EvidenceReader, EvidenceWriter, InMemoryStorage,
    LegislatureCounts, LegislatureReader, LegislatureWriter, LinkReader, LinkWriter,
    SCHEMA_VERSION, Storage,
};
use crate::uslm::bill_parser::Bill;

pub struct SqliteStorage {
    conn: Connection,
    metadata: DatasetMetadata,
}

/// The columns of `element_index`, in the order the insert takes them.
const ELEMENT_INDEX_INSERT: &str = "INSERT OR REPLACE INTO element_index \
     (work, date, path, node_type, heading, chapeau, content, proviso, continuation, ordinal) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)";

/// One row of the element index: every searchable field of one node, plus
/// where it sits in a document-order walk.
///
/// A struct rather than a tuple because two call sites write these rows, and a
/// positional tuple let the two drift apart without the compiler noticing.
struct ElementRow<'a> {
    path: &'a str,
    node_type: &'a str,
    heading: Option<&'a str>,
    chapeau: Option<&'a str>,
    content: Option<&'a str>,
    proviso: Option<&'a str>,
    continuation: Option<&'a str>,
    ordinal: i64,
}

impl ElementRow<'_> {
    fn execute(
        &self,
        stmt: &mut rusqlite::Statement,
        id: &ExpressionId,
    ) -> Result<(), DatasetError> {
        stmt.execute(params![
            id.work.as_str(),
            &id.at,
            self.path,
            self.node_type,
            self.heading,
            self.chapeau,
            self.content,
            self.proviso,
            self.continuation,
            self.ordinal,
        ])?;
        Ok(())
    }
}

/// The name of a target's variant, for the tag column.
fn target_tag(target: &Target) -> &'static str {
    match target {
        Target::Provision(_) => "provision",
        Target::Expression(_) => "expression",
        Target::Change { .. } => "change",
        Target::External { .. } => "external",
    }
}

/// One row of `links`: the record as JSON, plus the columns promoted out of it.
///
/// A struct rather than a tuple because two call sites write these rows — one
/// link at a time, and the bulk save — and a positional tuple let the two drift
/// apart without the compiler noticing, the same reason [`ElementRow`] exists.
struct LinkRow {
    id: String,
    kind: String,
    subject_tag: String,
    subject_json: String,
    object_tag: String,
    object_json: String,
    provenance_json: String,
    payload_json: Option<String>,
    subject_work: Option<String>,
    subject_path: Option<String>,
    subject_from_date: Option<String>,
    subject_to_date: Option<String>,
    object_reference: Option<String>,
}

const LINK_INSERT: &str = "INSERT OR REPLACE INTO links \
     (id, kind, subject_tag, subject_json, object_tag, object_json, provenance_json, \
      payload_json, subject_work, subject_path, subject_from_date, subject_to_date, \
      object_reference) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)";

impl LinkRow {
    fn new(id: &str, link: &Link) -> Result<Self, DatasetError> {
        // Promoted columns, derived from the subject on write. Only a change
        // and a provision carry a path; only a change carries a pair.
        let (work, path, from_date, to_date) = match &link.subject {
            Target::Change {
                work,
                path,
                from_date,
                to_date,
            } => (
                Some(work.to_string()),
                Some(path.clone()),
                Some(from_date.clone()),
                Some(to_date.clone()),
            ),
            Target::Provision(path) => (None, Some(path.clone()), None, None),
            _ => (None, None, None, None),
        };

        Ok(Self {
            id: id.to_string(),
            kind: link.kind.0.clone(),
            subject_tag: target_tag(&link.subject).to_string(),
            subject_json: serde_json::to_string(&link.subject)?,
            object_tag: target_tag(&link.object).to_string(),
            object_json: serde_json::to_string(&link.object)?,
            provenance_json: serde_json::to_string(&link.provenance)?,
            payload_json: link
                .payload
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            subject_work: work,
            subject_path: path,
            subject_from_date: from_date,
            subject_to_date: to_date,
            object_reference: match &link.object {
                Target::External { reference, .. } => Some(reference.clone()),
                _ => None,
            },
        })
    }

    fn bind(&self, stmt: &mut rusqlite::Statement) -> Result<(), DatasetError> {
        stmt.execute(params![
            self.id,
            self.kind,
            self.subject_tag,
            self.subject_json,
            self.object_tag,
            self.object_json,
            self.provenance_json,
            self.payload_json,
            self.subject_work,
            self.subject_path,
            self.subject_from_date,
            self.subject_to_date,
            self.object_reference,
        ])?;
        Ok(())
    }

    fn execute(&self, conn: &Connection) -> Result<(), DatasetError> {
        let mut stmt = conn.prepare(LINK_INSERT)?;
        self.bind(&mut stmt)
    }
}

/// Refuse a dataset this build cannot read.
///
/// Datasets are rebuilt rather than migrated, so a schema change is a clean
/// break. That is only safe if the break is loud: `CREATE TABLE IF NOT EXISTS`
/// would otherwise graft this build's tables onto an older file, and every
/// query against the new tables would return nothing. An empty answer that
/// means "wrong schema" is the failure this whole model exists to prevent.
///
/// A file with no `schema_version` table is new, and gets this build's schema.
fn check_schema_version(conn: &Connection) -> Result<(), DatasetError> {
    let found: Option<i32> = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .ok();

    match found {
        Some(version) if version != SCHEMA_VERSION => Err(DatasetError::SchemaVersionMismatch {
            found: version,
            expected: SCHEMA_VERSION,
        }),
        _ => Ok(()),
    }
}

/// Refuse a search index that predates the columns this build searches.
///
/// `CREATE TABLE IF NOT EXISTS` leaves an existing table alone, so a file built
/// before the index covered every text field keeps its narrower table. Reading
/// it would answer a search from two fields out of five and say nothing about
/// the other three, which is the silence #82 exists to stop. The schema version
/// does not catch this, because the compact JSON shares that number and its own
/// contents are unaffected.
fn check_search_index(conn: &Connection, path: &str) -> Result<(), DatasetError> {
    let table_exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'element_index'",
            [],
            |row| row.get::<_, i32>(0),
        )
        .is_ok();
    if !table_exists {
        return Ok(());
    }

    let mut stmt = conn.prepare("PRAGMA table_info(element_index)")?;
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;

    let complete = ["chapeau", "proviso", "continuation", "ordinal"]
        .iter()
        .all(|wanted| columns.iter().any(|held| held == wanted));

    // An index keyed by path holds one row per path, so it is missing a row for
    // every provision that shares one. The columns alone cannot show that, and
    // the remedy is the same: rebuild it.
    let keyed_by_position: bool = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'element_index'",
            [],
            |row| row.get::<_, String>(0),
        )
        .map(|sql| sql.contains("PRIMARY KEY (work, date, ordinal)"))
        .unwrap_or(false);

    if complete && keyed_by_position {
        Ok(())
    } else {
        Err(DatasetError::StaleSearchIndex {
            path: path.to_string(),
        })
    }
}

impl SqliteStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DatasetError> {
        let shown = path.as_ref().display().to_string();
        let conn = Connection::open(path)?;
        // Check before init_schema, which would otherwise add this build's
        // tables to an older file and leave a hybrid that answers queries with
        // nothing rather than saying it cannot read them.
        check_schema_version(&conn)?;
        check_search_index(&conn, &shown)?;
        let mut storage = Self {
            conn,
            metadata: DatasetMetadata::default(),
        };
        storage.init_schema()?;
        storage.metadata = storage.load_metadata().unwrap_or_default();
        Ok(storage)
    }

    pub fn open_in_memory() -> Result<Self, DatasetError> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn,
            metadata: DatasetMetadata::default(),
        };
        storage.init_schema()?;
        Ok(storage)
    }

    pub fn new_with_metadata(metadata: DatasetMetadata) -> Result<Self, DatasetError> {
        let conn = Connection::open_in_memory()?;
        let storage = Self { conn, metadata };
        storage.init_schema()?;
        storage.save_metadata()?;
        Ok(storage)
    }

    fn save_metadata(&self) -> Result<(), DatasetError> {
        let mut stmt = self
            .conn
            .prepare("INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)")?;
        stmt.execute(params!["name", &self.metadata.name])?;
        stmt.execute(params!["description", &self.metadata.description])?;
        stmt.execute(params!["author", &self.metadata.author])?;
        stmt.execute(params![
            "source_urls",
            serde_json::to_string(&self.metadata.source_urls)?
        ])?;
        stmt.execute(params!["license", &self.metadata.license])?;
        stmt.execute(params!["version", &self.metadata.version])?;
        // Only written when there is one, so "declared nothing" and "declared
        // an empty scope" stay distinguishable on disk.
        if let Some(declaration) = &self.metadata.declaration {
            stmt.execute(params!["declaration", serde_json::to_string(declaration)?])?;
        }
        Ok(())
    }

    fn init_schema(&self) -> Result<(), DatasetError> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY
            );

            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            -- One work as it read on one date. There is no table of release
            -- dates above this: a date owns nothing, so two documents
            -- published on unrelated days need share nothing.
            CREATE TABLE IF NOT EXISTS expressions (
                work TEXT NOT NULL,
                date TEXT NOT NULL,
                label TEXT,
                element_json TEXT NOT NULL,
                PRIMARY KEY (work, date)
            );

            CREATE TABLE IF NOT EXISTS element_index (
                work TEXT NOT NULL,
                date TEXT NOT NULL,
                path TEXT NOT NULL,
                -- The node type as the producer wrote it: `uscode.section`,
                -- `judicial.opinion`. An open namespaced string, so a class this
                -- build has never seen is stored and read back unchanged (#129).
                node_type TEXT,
                -- One column per variant of TextContentField. Indexing only
                -- some of them made their text unfindable, and silently, which
                -- reads to a searcher as the law not being there (#82).
                heading TEXT,
                chapeau TEXT,
                content TEXT,
                proviso TEXT,
                continuation TEXT,
                -- Position in a document-order walk, so search results can come
                -- back in the order a reader meets the provisions, matching the
                -- in-memory backend.
                ordinal INTEGER NOT NULL,
                -- Keyed by position, not by path. A path can name more than one
                -- provision (`docs/adr/0001`), and keying by path meant the
                -- second silently replaced the first: 16 of ~57,000 rows lost
                -- per title 26 expression, and its text unfindable (#77).
                PRIMARY KEY (work, date, ordinal)
            );

            CREATE TABLE IF NOT EXISTS bills (
                bill_id TEXT PRIMARY KEY,
                data_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS members (
                bioguide_id TEXT PRIMARY KEY,
                data_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sponsors (
                bill_id TEXT PRIMARY KEY,
                data_json TEXT NOT NULL
            );

            -- One table for every kind of link, including kinds this build
            -- has never seen. A schema whose only link table has columns for
            -- bills can hold exactly one kind, which is the limitation
            -- docs/adr/0004 exists to remove.
            CREATE TABLE IF NOT EXISTS links (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                subject_tag TEXT NOT NULL,
                subject_json TEXT NOT NULL,
                object_tag TEXT NOT NULL,
                object_json TEXT NOT NULL,
                provenance_json TEXT NOT NULL,
                payload_json TEXT,
                -- Promoted out of the JSON so the queries LinkReader answers
                -- can be indexed. The JSON is the record; these are derived
                -- from it on write and nothing reads them as the truth.
                subject_work TEXT,
                subject_path TEXT,
                subject_from_date TEXT,
                subject_to_date TEXT,
                object_reference TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_expr_work ON expressions(work);
            -- Finding a path is now a keyed lookup rather than a scan that
            -- deserializes one whole tree per date.
            CREATE INDEX IF NOT EXISTS idx_elem_path ON element_index(path);
            -- Verbatim model replies, by the hash of their own text. Append
            -- only: a reply whose statement was later superseded is kept,
            -- because deleting it destroys the trail it exists to create.
            CREATE TABLE IF NOT EXISTS model_replies (
                id TEXT PRIMARY KEY,
                reply TEXT NOT NULL
            );

            -- A namespace is a prefix of a kind, so one index serves both.
            CREATE INDEX IF NOT EXISTS idx_links_kind ON links(kind);
            CREATE INDEX IF NOT EXISTS idx_links_subject_path ON links(subject_path);
            CREATE INDEX IF NOT EXISTS idx_links_pair
                ON links(subject_work, subject_from_date, subject_to_date);
            CREATE INDEX IF NOT EXISTS idx_links_object_ref ON links(object_reference);

            -- Normalized votes
            CREATE TABLE IF NOT EXISTS roll_calls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                bill_id TEXT NOT NULL,
                congress INTEGER NOT NULL,
                session INTEGER NOT NULL,
                roll_number INTEGER NOT NULL,
                date TEXT NOT NULL,
                question TEXT NOT NULL,
                result TEXT NOT NULL,
                yea_count INTEGER,
                nay_count INTEGER,
                not_voting_count INTEGER,
                present_count INTEGER
            );

            CREATE TABLE IF NOT EXISTS member_votes (
                roll_call_id INTEGER NOT NULL,
                bioguide_id TEXT NOT NULL,
                position TEXT NOT NULL,
                PRIMARY KEY (roll_call_id, bioguide_id),
                FOREIGN KEY (roll_call_id) REFERENCES roll_calls(id)
            );

            CREATE INDEX IF NOT EXISTS idx_member_votes_bio ON member_votes(bioguide_id);
            CREATE INDEX IF NOT EXISTS idx_roll_calls_bill ON roll_calls(bill_id);
            "#,
        )?;

        // Set schema version if not exists
        self.conn.execute(
            "INSERT OR IGNORE INTO schema_version (version) VALUES (?1)",
            params![SCHEMA_VERSION],
        )?;

        Ok(())
    }

    pub fn save_from_memory(&mut self, storage: &InMemoryStorage) -> Result<(), DatasetError> {
        let tx = self.conn.transaction()?;

        // Save metadata
        {
            let mut stmt =
                tx.prepare("INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)")?;
            stmt.execute(params!["name", &storage.metadata.name])?;
            stmt.execute(params!["description", &storage.metadata.description])?;
            stmt.execute(params!["author", &storage.metadata.author])?;
            stmt.execute(params![
                "source_urls",
                serde_json::to_string(&storage.metadata.source_urls)?
            ])?;
            stmt.execute(params!["license", &storage.metadata.license])?;
            stmt.execute(params!["version", &storage.metadata.version])?;
            if let Some(declaration) = &storage.metadata.declaration {
                stmt.execute(params!["declaration", serde_json::to_string(declaration)?])?;
            }
        }

        // Save expressions
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO expressions (work, date, label, element_json) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for expression in storage.all_expressions() {
                let element_json = serde_json::to_string(&expression.root)?;
                stmt.execute(params![
                    expression.id.work.as_str(),
                    &expression.id.at,
                    &expression.label,
                    element_json
                ])?;
            }
        }

        // Save element index for each expression (batch collect then insert)
        {
            let mut stmt = tx.prepare(ELEMENT_INDEX_INSERT)?;
            for expression in storage.all_expressions() {
                let mut rows = Vec::new();
                Self::collect_element_rows(&expression.root, &mut rows);
                for row in rows {
                    row.execute(&mut stmt, &expression.id)?;
                }
            }
        }

        // Save bills
        {
            let mut stmt =
                tx.prepare("INSERT OR REPLACE INTO bills (bill_id, data_json) VALUES (?1, ?2)")?;
            for (bill_id, bill) in &storage.bills {
                let json = serde_json::to_string(bill)?;
                stmt.execute(params![bill_id, json])?;
            }
        }

        // Save members
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO members (bioguide_id, data_json) VALUES (?1, ?2)",
            )?;
            for (bioguide_id, member) in &storage.members {
                let json = serde_json::to_string(member)?;
                stmt.execute(params![bioguide_id, json])?;
            }
        }

        // Save sponsors
        {
            let mut stmt =
                tx.prepare("INSERT OR REPLACE INTO sponsors (bill_id, data_json) VALUES (?1, ?2)")?;
            for (bill_id, info) in &storage.sponsors {
                let json = serde_json::to_string(info)?;
                stmt.execute(params![bill_id, json])?;
            }
        }

        // Save replies. Append-only, so no DELETE: a reply the incoming
        // storage no longer references is still a record that it was said.
        {
            let mut stmt =
                tx.prepare("INSERT OR REPLACE INTO model_replies (id, reply) VALUES (?1, ?2)")?;
            for (id, reply) in &storage.replies {
                stmt.execute(params![id, reply])?;
            }
        }

        // Save links
        {
            tx.execute("DELETE FROM links", [])?;
            let mut stmt = tx.prepare(LINK_INSERT)?;
            for (id, link) in &storage.links {
                LinkRow::new(id, link)?.bind(&mut stmt)?;
            }
        }
        // Save bill_votes (normalized)
        {
            tx.execute("DELETE FROM member_votes", [])?;
            tx.execute("DELETE FROM roll_calls", [])?;

            let mut rc_stmt = tx.prepare(
                "INSERT INTO roll_calls (bill_id, congress, session, roll_number, date, question, result, yea_count, nay_count, not_voting_count, present_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            let mut mv_stmt = tx.prepare(
                "INSERT INTO member_votes (roll_call_id, bioguide_id, position) VALUES (?1, ?2, ?3)",
            )?;

            for votes in storage.bill_votes.values() {
                for rc in &votes.roll_calls {
                    rc_stmt.execute(params![
                        &votes.bill_id,
                        rc.congress,
                        rc.session,
                        rc.roll_number,
                        &rc.date,
                        &rc.question,
                        &rc.result,
                        rc.yea_count,
                        rc.nay_count,
                        rc.not_voting_count,
                        rc.present_count,
                    ])?;

                    let rc_id = tx.last_insert_rowid();
                    for mv in &rc.member_votes {
                        let position = format!("{:?}", mv.position);
                        mv_stmt.execute(params![rc_id, &mv.bioguide_id, position])?;
                    }
                }
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Collect node data into rows for batch insert (no recursion overhead per-insert)
    #[allow(clippy::type_complexity)]
    fn collect_element_rows<'a>(element: &'a DocumentNode, rows: &mut Vec<ElementRow<'a>>) {
        let text = |field: &'a Option<Arc<str>>| field.as_ref().map(|s| s.as_ref());
        rows.push(ElementRow {
            path: element.data.path.as_ref(),
            node_type: element.data.node_type.as_str(),
            heading: text(&element.data.heading),
            chapeau: text(&element.data.chapeau),
            content: text(&element.data.content),
            proviso: text(&element.data.proviso),
            continuation: text(&element.data.continuation),
            ordinal: rows.len() as i64,
        });

        for child in &element.children {
            Self::collect_element_rows(child, rows);
        }
    }

    fn index_element(
        stmt: &mut rusqlite::Statement,
        id: &ExpressionId,
        element: &DocumentNode,
    ) -> Result<(), DatasetError> {
        let mut rows = Vec::new();
        Self::collect_element_rows(element, &mut rows);
        for row in rows {
            row.execute(stmt, id)?;
        }
        Ok(())
    }

    /// Load a window of two expressions with the links about them
    ///
    /// Returns InMemoryStorage with only the specified expressions and their links.
    /// Bills, members, sponsors are NOT loaded - query them from storage as needed.
    pub fn load_window(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<InMemoryStorage, DatasetError> {
        let metadata = self.load_metadata()?;

        let (from_e, to_e) = require_same_work(self, from, to)?;

        let mut expressions = ExpressionsByWork::new();
        let by_date = expressions.entry(from.work.clone()).or_default();
        by_date.insert(from_e.id.at.clone(), from_e);
        by_date.insert(to_e.id.at.clone(), to_e);

        // Load links about this pair only
        let links = self
            .links_for_pair(from, to)?
            .into_iter()
            .map(|link| (link.id(), link))
            .collect();
        let replies = self.load_replies()?;

        let mut storage = InMemoryStorage::from_parts(
            metadata,
            expressions,
            HashMap::new(), // bills - query from storage
            links,
            replies,
            HashMap::new(), // members - query from storage
            HashMap::new(), // sponsors - query from storage
            HashMap::new(), // bill_votes - query from storage
            StringInterner::new(),
        );
        storage.intern_strings();

        Ok(storage)
    }

    /// Load entire database into memory
    pub fn to_memory(&self) -> Result<InMemoryStorage, DatasetError> {
        let metadata = self.load_metadata()?;
        let expressions = self.load_expressions()?;
        let bills = self.load_bills()?;
        let members = self.load_members()?;
        let sponsors = self.load_sponsors()?;
        let links = self.load_links()?;
        let replies = self.load_replies()?;
        let bill_votes = self.load_bill_votes()?;

        let mut storage = InMemoryStorage::from_parts(
            metadata,
            expressions,
            bills,
            links,
            replies,
            members,
            sponsors,
            bill_votes,
            StringInterner::new(),
        );
        storage.intern_strings();

        Ok(storage)
    }

    fn load_metadata(&self) -> Result<DatasetMetadata, DatasetError> {
        let mut stmt = self.conn.prepare("SELECT key, value FROM metadata")?;
        let mut rows = stmt.query([])?;

        let mut name = String::new();
        let mut description = String::new();
        let mut author = String::new();
        let mut source_urls = Vec::new();
        let mut license = String::new();
        let mut version = String::new();
        let mut declaration = None;

        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            match key.as_str() {
                "name" => name = value,
                "description" => description = value,
                "author" => author = value,
                "source_urls" => source_urls = serde_json::from_str(&value)?,
                "license" => license = value,
                "version" => version = value,
                "declaration" => declaration = Some(serde_json::from_str(&value)?),
                _ => {}
            }
        }

        Ok(DatasetMetadata {
            name,
            description,
            author,
            source_urls,
            license,
            version,
            declaration,
        })
    }

    fn load_expressions(&self) -> Result<ExpressionsByWork, DatasetError> {
        let mut stmt = self.conn.prepare(
            "SELECT work, date, label, element_json FROM expressions ORDER BY work, date",
        )?;
        let mut rows = stmt.query([])?;

        let mut expressions = ExpressionsByWork::new();
        while let Some(row) = rows.next()? {
            let work: String = row.get(0)?;
            let date: String = row.get(1)?;
            let label: Option<String> = row.get(2)?;
            let element_json: String = row.get(3)?;
            let root: DocumentNode = serde_json::from_str(&element_json)?;

            let id = ExpressionId::new(WorkId::new(work), date);
            expressions
                .entry(id.work.clone())
                .or_default()
                .insert(id.at.clone(), Expression { id, label, root });
        }

        Ok(expressions)
    }

    fn load_bills(&self) -> Result<HashMap<String, Bill>, DatasetError> {
        let mut stmt = self.conn.prepare("SELECT bill_id, data_json FROM bills")?;
        let mut rows = stmt.query([])?;

        let mut bills = HashMap::new();
        while let Some(row) = rows.next()? {
            let bill_id: String = row.get(0)?;
            let data_json: String = row.get(1)?;
            let bill: Bill = serde_json::from_str(&data_json)?;
            bills.insert(bill_id, bill);
        }

        Ok(bills)
    }

    fn load_members(&self) -> Result<HashMap<String, Member>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT bioguide_id, data_json FROM members")?;
        let mut rows = stmt.query([])?;

        let mut members = HashMap::new();
        while let Some(row) = rows.next()? {
            let bioguide_id: String = row.get(0)?;
            let data_json: String = row.get(1)?;
            let member: Member = serde_json::from_str(&data_json)?;
            members.insert(bioguide_id, member);
        }

        Ok(members)
    }

    fn load_sponsors(&self) -> Result<HashMap<String, SponsorInfo>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT bill_id, data_json FROM sponsors")?;
        let mut rows = stmt.query([])?;

        let mut sponsors = HashMap::new();
        while let Some(row) = rows.next()? {
            let bill_id: String = row.get(0)?;
            let data_json: String = row.get(1)?;
            let info: SponsorInfo = serde_json::from_str(&data_json)?;
            sponsors.insert(bill_id, info);
        }

        Ok(sponsors)
    }

    /// Every verbatim model reply, by id.
    fn load_replies(&self) -> Result<std::collections::BTreeMap<String, String>, DatasetError> {
        let mut stmt = self.conn.prepare("SELECT id, reply FROM model_replies")?;
        let replies = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
        Ok(replies)
    }

    /// Every link in the database, by id.
    fn load_links(&self) -> Result<std::collections::BTreeMap<String, Link>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {LINK_COLUMNS} FROM links"))?;
        let links = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>("id")?, link_from_row(row)?))
            })?
            .collect::<Result<std::collections::BTreeMap<_, _>, _>>()?;
        Ok(links)
    }

    fn load_bill_votes(&self) -> Result<HashMap<String, BillVotes>, DatasetError> {
        // Load roll calls grouped by bill
        let mut rc_stmt = self.conn.prepare(
            "SELECT id, bill_id, congress, session, roll_number, date, question, result,
                    yea_count, nay_count, not_voting_count, present_count
             FROM roll_calls
             ORDER BY bill_id, id",
        )?;
        let mut rc_rows = rc_stmt.query([])?;

        let mut votes: HashMap<String, BillVotes> = HashMap::new();
        let mut rc_ids: Vec<(i64, String)> = Vec::new(); // (id, bill_id)

        while let Some(row) = rc_rows.next()? {
            let id: i64 = row.get(0)?;
            let bill_id: String = row.get(1)?;
            let congress: u16 = row.get(2)?;
            let session: u8 = row.get(3)?;
            let roll_number: u32 = row.get(4)?;
            let date: String = row.get(5)?;
            let question: String = row.get(6)?;
            let result: String = row.get(7)?;
            let yea_count: u32 = row.get(8)?;
            let nay_count: u32 = row.get(9)?;
            let not_voting_count: u32 = row.get(10)?;
            let present_count: u32 = row.get(11)?;

            let rc = HouseRollCall {
                congress,
                session,
                roll_number,
                date,
                question,
                result,
                yea_count,
                nay_count,
                not_voting_count,
                present_count,
                member_votes: Vec::new(), // Fill in below
            };

            votes
                .entry(bill_id.clone())
                .or_insert_with(|| BillVotes {
                    bill_id: bill_id.clone(),
                    roll_calls: Vec::new(),
                })
                .roll_calls
                .push(rc);

            rc_ids.push((id, bill_id));
        }

        // Load member votes for each roll call
        let mut mv_stmt = self
            .conn
            .prepare("SELECT bioguide_id, position FROM member_votes WHERE roll_call_id = ?1")?;

        for (rc_id, bill_id) in rc_ids {
            let mut mv_rows = mv_stmt.query(params![rc_id])?;
            let mut member_votes = Vec::new();

            while let Some(row) = mv_rows.next()? {
                let bioguide_id: String = row.get(0)?;
                let position_str: String = row.get(1)?;
                let position = match position_str.as_str() {
                    "Yea" => VotePosition::Yea,
                    "Nay" => VotePosition::Nay,
                    "Present" => VotePosition::Present,
                    _ => VotePosition::NotVoting,
                };
                member_votes.push(MemberVote {
                    bioguide_id,
                    position,
                });
            }

            // Find the roll call and set member_votes
            if let Some(bv) = votes.get_mut(&bill_id) {
                for rc in bv.roll_calls.iter_mut() {
                    if rc.member_votes.is_empty() {
                        rc.member_votes = member_votes;
                        break;
                    }
                }
            }
        }

        Ok(votes)
    }
}

impl SqliteStorage {
    /// Read one expression row, given a query that selects
    /// `work, date, label, element_json` and its parameters.
    fn expression_row(
        &self,
        sql: &str,
        bound: &[&dyn rusqlite::ToSql],
    ) -> Result<Option<Expression>, DatasetError> {
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = stmt.query(bound)?;

        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let work: String = row.get(0)?;
        let date: String = row.get(1)?;
        let label: Option<String> = row.get(2)?;
        let element_json: String = row.get(3)?;
        Ok(Some(Expression {
            id: ExpressionId::new(WorkId::new(work), date),
            label,
            root: serde_json::from_str(&element_json)?,
        }))
    }
}

impl DocumentReader for SqliteStorage {
    fn metadata(&self) -> &DatasetMetadata {
        &self.metadata
    }

    fn works(&self) -> Result<Vec<WorkId>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT work FROM expressions ORDER BY work")?;
        let mut rows = stmt.query([])?;

        let mut works = Vec::new();
        while let Some(row) = rows.next()? {
            let work: String = row.get(0)?;
            works.push(WorkId::new(work));
        }

        Ok(works)
    }

    fn expressions(&self, work: &WorkId) -> Result<Vec<ExpressionInfo>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT date, label FROM expressions WHERE work = ?1 ORDER BY date")?;
        let mut rows = stmt.query(params![work.as_str()])?;

        let mut expressions = Vec::new();
        while let Some(row) = rows.next()? {
            let date: String = row.get(0)?;
            let label: Option<String> = row.get(1)?;
            expressions.push(ExpressionInfo {
                id: ExpressionId::new(work.clone(), date),
                label,
            });
        }

        Ok(expressions)
    }

    fn get_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.expression_row(
            "SELECT work, date, label, element_json FROM expressions \
             WHERE work = ?1 AND date = ?2",
            &[&id.work.as_str(), &id.at],
        )
    }

    fn next_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.expression_row(
            "SELECT work, date, label, element_json FROM expressions \
             WHERE work = ?1 AND date > ?2 ORDER BY date LIMIT 1",
            &[&id.work.as_str(), &id.at],
        )
    }

    fn prev_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.expression_row(
            "SELECT work, date, label, element_json FROM expressions \
             WHERE work = ?1 AND date < ?2 ORDER BY date DESC LIMIT 1",
            &[&id.work.as_str(), &id.at],
        )
    }

    fn compute_diff(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<TreeDiff, DatasetError> {
        let (from_e, to_e) = require_same_work(self, from, to)?;
        Ok(TreeDiff::from_nodes(&from_e.root, &to_e.root))
    }

    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
        // Every text field, not a chosen few: a field left out of this query is
        // a field whose text reads as absent from the law (#82).
        //
        // The ordering reproduces the in-memory walk exactly, so the two
        // backends answer alike: work, then date, then document position, then
        // the field order that backend declares.
        let query_pattern = format!("%{}%", query.to_lowercase());
        let mut stmt = self.conn.prepare(
            "SELECT work, date, path, ordinal, 0 AS field_rank, 'heading' AS field, heading AS snippet
                 FROM element_index WHERE LOWER(heading) LIKE ?1
             UNION ALL
             SELECT work, date, path, ordinal, 1, 'chapeau', chapeau
                 FROM element_index WHERE LOWER(chapeau) LIKE ?1
             UNION ALL
             SELECT work, date, path, ordinal, 2, 'content', content
                 FROM element_index WHERE LOWER(content) LIKE ?1
             UNION ALL
             SELECT work, date, path, ordinal, 3, 'proviso', proviso
                 FROM element_index WHERE LOWER(proviso) LIKE ?1
             UNION ALL
             SELECT work, date, path, ordinal, 4, 'continuation', continuation
                 FROM element_index WHERE LOWER(continuation) LIKE ?1
             ORDER BY work, date, ordinal, field_rank",
        )?;
        let mut rows = stmt.query(params![query_pattern])?;

        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            let work: String = row.get(0)?;
            let date: String = row.get(1)?;
            let path: String = row.get(2)?;
            let field: String = row.get(5)?;
            let snippet: Option<String> = row.get(6)?;
            if let Some(snippet) = snippet {
                results.push(SearchResult {
                    expression: ExpressionId::new(WorkId::new(work), date),
                    path,
                    field,
                    snippet,
                });
            }
        }

        Ok(results)
    }

    fn find_nodes(&self, path: &str) -> Result<Vec<(ExpressionId, DocumentNode)>, DatasetError> {
        // The index says which expressions hold this path; only those are read.
        // Ordered so both backends answer alike.
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT work, date FROM element_index WHERE path = ?1 ORDER BY work, date",
        )?;
        let mut rows = stmt.query(params![path])?;

        let mut wanted = Vec::new();
        while let Some(row) = rows.next()? {
            let work: String = row.get(0)?;
            let date: String = row.get(1)?;
            wanted.push(ExpressionId::new(WorkId::new(work), date));
        }

        let mut results = Vec::new();
        for id in wanted {
            if let Some(expression) = self.get_expression(&id)? {
                // A path can name more than one provision within an expression.
                for found in expression.root.find_all(path) {
                    results.push((id.clone(), found.clone()));
                }
            }
        }

        Ok(results)
    }

    fn has_node(&self, path: &str) -> Result<bool, DatasetError> {
        // `idx_elem_path` answers this on its own, so no `element_json` is read.
        // The index holds a row per provision, and one row is enough: the
        // question is whether any provision sits at the path.
        let mut stmt = self
            .conn
            .prepare("SELECT 1 FROM element_index WHERE path = ?1")?;
        Ok(stmt.exists(params![path])?)
    }
}

/// Rebuild a link from one row of `links`.
///
/// The JSON columns are the record. The promoted columns exist only so the
/// queries can be indexed, and are never read back here.
fn link_from_row(row: &rusqlite::Row) -> Result<Link, rusqlite::Error> {
    let subject_json: String = row.get("subject_json")?;
    let object_json: String = row.get("object_json")?;
    let provenance_json: String = row.get("provenance_json")?;
    let payload_json: Option<String> = row.get("payload_json")?;
    let kind: String = row.get("kind")?;

    // A function rather than a closure: it is called at four different types,
    // and a closure fixes itself to the first one.
    fn parse<T: serde::de::DeserializeOwned>(
        text: &str,
        column: &str,
    ) -> Result<T, rusqlite::Error> {
        serde_json::from_str(text).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::other(format!("{column}: {e}"))),
            )
        })
    }

    Ok(Link {
        subject: parse(&subject_json, "subject_json")?,
        kind: LinkKind::new(kind),
        object: parse(&object_json, "object_json")?,
        provenance: parse(&provenance_json, "provenance_json")?,
        payload: match payload_json {
            Some(text) => Some(parse(&text, "payload_json")?),
            None => None,
        },
    })
}

impl SqliteStorage {
    /// How many rows one of this schema's tables holds.
    ///
    /// `table` is always a literal from this file — a table name cannot be a
    /// bound parameter, so it must never come from a caller.
    fn count_rows(&self, table: &str) -> Result<usize, DatasetError> {
        let count: i64 =
            self.conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })?;
        Ok(count as usize)
    }

    /// Run one link query and collect the rows.
    fn query_links<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<Link>, DatasetError> {
        let mut stmt = self.conn.prepare(sql)?;
        let links = stmt
            .query_map(params, link_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(links)
    }
}

const LINK_COLUMNS: &str =
    "id, kind, subject_tag, subject_json, object_tag, object_json, provenance_json, payload_json";

impl LinkReader for SqliteStorage {
    fn links_for_path(&self, path: &str) -> Result<Vec<Link>, DatasetError> {
        self.query_links(
            &format!("SELECT {LINK_COLUMNS} FROM links WHERE subject_path = ?1"),
            params![path],
        )
    }

    fn links_for_pair(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<Vec<Link>, DatasetError> {
        self.query_links(
            &format!(
                "SELECT {LINK_COLUMNS} FROM links \
                 WHERE subject_work = ?1 AND subject_from_date = ?2 AND subject_to_date = ?3"
            ),
            params![from.work.as_str(), &from.at, &to.at],
        )
    }

    fn links_by_kind(&self, kind: &str) -> Result<Vec<Link>, DatasetError> {
        self.query_links(
            &format!("SELECT {LINK_COLUMNS} FROM links WHERE kind = ?1"),
            params![kind],
        )
    }

    fn links_by_namespace(&self, namespace: &str) -> Result<Vec<Link>, DatasetError> {
        // A namespace is the part of a kind before the dot, so the index on
        // kind answers this too.
        self.query_links(
            &format!("SELECT {LINK_COLUMNS} FROM links WHERE kind LIKE ?1"),
            params![format!("{namespace}.%")],
        )
    }

    fn links_for_object_prefix(&self, prefix: &str) -> Result<Vec<Link>, DatasetError> {
        self.query_links(
            &format!("SELECT {LINK_COLUMNS} FROM links WHERE object_reference LIKE ?1"),
            params![format!("{}%", prefix.replace('%', "\\%"))],
        )
    }

    fn count_links_by_kind(&self) -> Result<BTreeMap<String, usize>, DatasetError> {
        // One grouped count. Building the links to count them would read every
        // subject, object, and provenance document in the table.
        let mut stmt = self
            .conn
            .prepare("SELECT kind, COUNT(*) FROM links GROUP BY kind ORDER BY kind")?;
        let counts = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as usize)))?
            .collect::<Result<BTreeMap<String, usize>, _>>()?;
        Ok(counts)
    }

    fn link_pairs(&self) -> Result<Vec<ExpressionPair>, DatasetError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT subject_work, subject_from_date, subject_to_date FROM links \
             WHERE subject_work IS NOT NULL \
             ORDER BY subject_work, subject_from_date, subject_to_date",
        )?;
        let pairs = stmt
            .query_map([], |row| {
                let work: String = row.get(0)?;
                let from: String = row.get(1)?;
                let to: String = row.get(2)?;
                Ok((
                    ExpressionId::new(WorkId::new(work.clone()), from),
                    ExpressionId::new(WorkId::new(work), to),
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(pairs)
    }
}

impl EvidenceReader for SqliteStorage {
    fn get_reply(&self, id: &str) -> Result<Option<String>, DatasetError> {
        Ok(self
            .conn
            .query_row(
                "SELECT reply FROM model_replies WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .ok())
    }

    fn replies(&self) -> Result<Vec<String>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM model_replies ORDER BY id")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    }

    fn count_replies(&self) -> Result<usize, DatasetError> {
        self.count_rows("model_replies")
    }
}

impl EvidenceWriter for SqliteStorage {
    fn add_reply(&mut self, reply: &str) -> Result<String, DatasetError> {
        let id = crate::link::reply_id(reply);
        self.conn.execute(
            "INSERT OR REPLACE INTO model_replies (id, reply) VALUES (?1, ?2)",
            params![&id, reply],
        )?;
        Ok(id)
    }
}

impl LegislatureReader for SqliteStorage {
    fn get_bill(&self, id: &str) -> Result<Option<Bill>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM bills WHERE bill_id = ?1")?;
        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            let data_json: String = row.get(0)?;
            let bill: Bill = serde_json::from_str(&data_json)?;
            Ok(Some(bill))
        } else {
            Ok(None)
        }
    }

    fn list_bill_ids(&self) -> Result<Vec<String>, DatasetError> {
        let mut stmt = self.conn.prepare("SELECT bill_id FROM bills")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    }

    fn get_member(&self, bioguide_id: &str) -> Result<Option<Member>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM members WHERE bioguide_id = ?1")?;
        let mut rows = stmt.query(params![bioguide_id])?;

        if let Some(row) = rows.next()? {
            let data_json: String = row.get(0)?;
            let member: Member = serde_json::from_str(&data_json)?;
            Ok(Some(member))
        } else {
            Ok(None)
        }
    }

    fn get_sponsor_info(&self, bill_id: &str) -> Result<Option<SponsorInfo>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM sponsors WHERE bill_id = ?1")?;
        let mut rows = stmt.query(params![bill_id])?;

        if let Some(row) = rows.next()? {
            let data_json: String = row.get(0)?;
            let info: SponsorInfo = serde_json::from_str(&data_json)?;
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    fn get_bill_votes(&self, bill_id: &str) -> Result<Option<BillVotes>, DatasetError> {
        let all = self.load_bill_votes()?;
        Ok(all.get(bill_id).cloned())
    }

    fn votes_by_member(
        &self,
        bioguide_id: &str,
    ) -> Result<Vec<(HouseRollCall, VotePosition)>, DatasetError> {
        // Join member_votes with roll_calls
        let mut stmt = self.conn.prepare(
            "SELECT rc.id, rc.bill_id, rc.congress, rc.session, rc.roll_number, rc.date,
                    rc.question, rc.result, rc.yea_count, rc.nay_count, rc.not_voting_count,
                    rc.present_count, mv.position
             FROM roll_calls rc
             JOIN member_votes mv ON rc.id = mv.roll_call_id
             WHERE mv.bioguide_id = ?1",
        )?;
        let mut rows = stmt.query(params![bioguide_id])?;

        let mut results = Vec::new();
        let mut rc_ids = Vec::new();

        while let Some(row) = rows.next()? {
            let rc_id: i64 = row.get(0)?;
            let congress: u16 = row.get(2)?;
            let session: u8 = row.get(3)?;
            let roll_number: u32 = row.get(4)?;
            let date: String = row.get(5)?;
            let question: String = row.get(6)?;
            let result: String = row.get(7)?;
            let yea_count: u32 = row.get(8)?;
            let nay_count: u32 = row.get(9)?;
            let not_voting_count: u32 = row.get(10)?;
            let present_count: u32 = row.get(11)?;
            let position_str: String = row.get(12)?;

            let position = match position_str.as_str() {
                "Yea" => VotePosition::Yea,
                "Nay" => VotePosition::Nay,
                "Present" => VotePosition::Present,
                _ => VotePosition::NotVoting,
            };

            let rc = HouseRollCall {
                congress,
                session,
                roll_number,
                date,
                question,
                result,
                yea_count,
                nay_count,
                not_voting_count,
                present_count,
                member_votes: Vec::new(), // We'll fill this in
            };

            results.push((rc, position));
            rc_ids.push(rc_id);
        }

        // Load all member votes for each roll call
        let mut mv_stmt = self
            .conn
            .prepare("SELECT bioguide_id, position FROM member_votes WHERE roll_call_id = ?1")?;

        for (i, rc_id) in rc_ids.iter().enumerate() {
            let mut mv_rows = mv_stmt.query(params![rc_id])?;
            while let Some(row) = mv_rows.next()? {
                let bio: String = row.get(0)?;
                let pos_str: String = row.get(1)?;
                let pos = match pos_str.as_str() {
                    "Yea" => VotePosition::Yea,
                    "Nay" => VotePosition::Nay,
                    "Present" => VotePosition::Present,
                    _ => VotePosition::NotVoting,
                };
                results[i].0.member_votes.push(MemberVote {
                    bioguide_id: bio,
                    position: pos,
                });
            }
        }

        Ok(results)
    }

    fn legislature_counts(&self) -> Result<LegislatureCounts, DatasetError> {
        Ok(LegislatureCounts {
            bills: self.count_rows("bills")?,
            members: self.count_rows("members")?,
            sponsors: self.count_rows("sponsors")?,
            roll_calls: self.count_rows("roll_calls")?,
            member_votes: self.count_rows("member_votes")?,
        })
    }
}

impl DocumentWriter for SqliteStorage {
    fn set_metadata(&mut self, metadata: DatasetMetadata) {
        self.metadata = metadata;
        let _ = self.save_metadata();
    }

    fn add_expression(&mut self, expression: Expression) -> Result<(), DatasetError> {
        let element_json = serde_json::to_string(&expression.root)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO expressions (work, date, label, element_json) VALUES (?1, ?2, ?3, ?4)",
            params![
                expression.id.work.as_str(),
                &expression.id.at,
                &expression.label,
                element_json
            ],
        )?;

        // Index elements
        let mut stmt = self.conn.prepare(ELEMENT_INDEX_INSERT)?;
        Self::index_element(&mut stmt, &expression.id, &expression.root)?;

        Ok(())
    }
}

impl LinkWriter for SqliteStorage {
    fn add_link(&mut self, link: Link) -> Result<(), DatasetError> {
        let id = link.id();

        // A human's verdict is not overwritten by a machine restating the fact.
        let stored: Option<String> = self
            .conn
            .query_row(
                "SELECT provenance_json FROM links WHERE id = ?1",
                params![&id],
                |row| row.get(0),
            )
            .ok();
        if let Some(text) = stored
            && let Ok(provenance) = serde_json::from_str::<Provenance>(&text)
            && provenance.verification.is_human_touched()
        {
            return Ok(());
        }

        let row = LinkRow::new(&id, &link)?;
        row.execute(&self.conn)
    }
}

impl LegislatureWriter for SqliteStorage {
    fn add_bill(&mut self, bill: Bill) -> Result<(), DatasetError> {
        let json = serde_json::to_string(&bill)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO bills (bill_id, data_json) VALUES (?1, ?2)",
            params![&bill.bill_id, json],
        )?;
        Ok(())
    }

    fn add_member(&mut self, member: Member) -> Result<(), DatasetError> {
        let json = serde_json::to_string(&member)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO members (bioguide_id, data_json) VALUES (?1, ?2)",
            params![&member.bioguide_id, json],
        )?;
        Ok(())
    }

    fn add_sponsor_info(&mut self, info: SponsorInfo) -> Result<(), DatasetError> {
        let json = serde_json::to_string(&info)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO sponsors (bill_id, data_json) VALUES (?1, ?2)",
            params![&info.bill_id, json],
        )?;
        Ok(())
    }

    fn add_bill_votes(&mut self, votes: BillVotes) -> Result<(), DatasetError> {
        for rc in &votes.roll_calls {
            self.conn.execute(
                "INSERT INTO roll_calls (bill_id, congress, session, roll_number, date, question, result, yea_count, nay_count, not_voting_count, present_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    &votes.bill_id,
                    rc.congress,
                    rc.session,
                    rc.roll_number,
                    &rc.date,
                    &rc.question,
                    &rc.result,
                    rc.yea_count,
                    rc.nay_count,
                    rc.not_voting_count,
                    rc.present_count,
                ],
            )?;

            let rc_id = self.conn.last_insert_rowid();
            for mv in &rc.member_votes {
                let position = format!("{:?}", mv.position);
                self.conn.execute(
                    "INSERT INTO member_votes (roll_call_id, bioguide_id, position) VALUES (?1, ?2, ?3)",
                    params![rc_id, &mv.bioguide_id, position],
                )?;
            }
        }
        Ok(())
    }
}

impl SqliteStorage {
    /// True when any legislative table holds a row.
    ///
    /// A dataset that was never given bills, sponsors, members, or votes has
    /// no legislature to offer, even though the tables exist because every
    /// dataset shares one schema today.
    fn holds_legislature(&self) -> Result<bool, DatasetError> {
        for table in ["bills", "members", "sponsors", "roll_calls"] {
            let present: bool = self.conn.query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} LIMIT 1)"),
                [],
                |row| row.get(0),
            )?;
            if present {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Storage for SqliteStorage {
    fn legislature(&self) -> Option<&dyn LegislatureReader> {
        let declared = self
            .metadata
            .declaration
            .as_ref()
            .is_some_and(|d| d.declares_namespace(crate::link::LinkKind::LEGISLATURE));
        // A database error here means we cannot show legislative material, so
        // the honest answer is that this dataset offers none. A declaration
        // still counts: it is a statement, not a read that can fail.
        let holds_legislature = self.holds_legislature().unwrap_or(false);
        (declared || holds_legislature).then_some(self as &dyn LegislatureReader)
    }
}
