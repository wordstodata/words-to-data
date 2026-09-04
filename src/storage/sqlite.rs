//! SQLite storage backend for datasets

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, params};

use crate::annotation::ChangeAnnotation;
use crate::congress::{BillVotes, HouseRollCall, Member, MemberVote, SponsorInfo, VotePosition};
use crate::dataset::{DatasetError, DatasetMetadata, SearchResult, VersionPair, VersionSnapshot};
use crate::diff::TreeDiff;
use crate::intern::StringInterner;
use crate::storage::{
    DocumentReader, DocumentWriter, InMemoryStorage, LegislatureReader, LegislatureWriter,
    LinkReader, LinkWriter, Storage, VersionInfo,
};
use crate::uslm::USLMElement;
use crate::uslm::bill_parser::Bill;

const SCHEMA_VERSION: i32 = 2;

pub struct SqliteStorage {
    conn: Connection,
    metadata: DatasetMetadata,
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

impl SqliteStorage {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DatasetError> {
        let conn = Connection::open(path)?;
        // Check before init_schema, which would otherwise add this build's
        // tables to an older file and leave a hybrid that answers queries with
        // nothing rather than saying it cannot read them.
        check_schema_version(&conn)?;
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

            CREATE TABLE IF NOT EXISTS versions (
                date TEXT PRIMARY KEY,
                label TEXT,
                element_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS element_index (
                version_date TEXT NOT NULL,
                path TEXT NOT NULL,
                element_type TEXT,
                heading TEXT,
                content TEXT,
                PRIMARY KEY (version_date, path)
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

            -- Normalized annotations
            CREATE TABLE IF NOT EXISTS annotations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_date TEXT NOT NULL,
                to_date TEXT NOT NULL,
                operation TEXT NOT NULL,
                bill_id TEXT NOT NULL,
                amendment_id TEXT NOT NULL,
                causative_text TEXT NOT NULL,
                status TEXT NOT NULL,
                confidence REAL,
                annotator TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                notes TEXT,
                reasoning TEXT
            );

            CREATE TABLE IF NOT EXISTS annotation_paths (
                annotation_id INTEGER NOT NULL,
                path TEXT NOT NULL,
                PRIMARY KEY (annotation_id, path),
                FOREIGN KEY (annotation_id) REFERENCES annotations(id)
            );

            CREATE INDEX IF NOT EXISTS idx_ann_dates ON annotations(from_date, to_date);
            CREATE INDEX IF NOT EXISTS idx_ann_bill ON annotations(bill_id);
            CREATE INDEX IF NOT EXISTS idx_ann_paths_path ON annotation_paths(path);

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
        }

        // Save versions
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO versions (date, label, element_json) VALUES (?1, ?2, ?3)",
            )?;
            for version in &storage.versions {
                let element_json = serde_json::to_string(&version.element)?;
                stmt.execute(params![&version.date, &version.label, element_json])?;
            }
        }

        // Save element index for each version (batch collect then insert)
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO element_index (version_date, path, element_type, heading, content) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for version in &storage.versions {
                let mut rows = Vec::new();
                Self::collect_element_rows(&version.date, &version.element, &mut rows);
                for (date, path, elem_type, heading, content) in rows {
                    stmt.execute(params![date, path, elem_type, heading, content])?;
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

        // Save annotations (normalized)
        {
            tx.execute("DELETE FROM annotation_paths", [])?;
            tx.execute("DELETE FROM annotations", [])?;

            let mut ann_stmt = tx.prepare(
                "INSERT INTO annotations (from_date, to_date, operation, bill_id, amendment_id, causative_text, status, confidence, annotator, timestamp, notes, reasoning) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            )?;
            let mut path_stmt =
                tx.prepare("INSERT INTO annotation_paths (annotation_id, path) VALUES (?1, ?2)")?;

            for ((from, to), annotations) in &storage.diff_annotations {
                for ann in annotations {
                    let operation = format!("{:?}", ann.operation);
                    let status = format!("{:?}", ann.metadata.status);
                    let timestamp = ann.metadata.timestamp.to_string();

                    ann_stmt.execute(params![
                        from,
                        to,
                        operation,
                        &ann.source_bill.bill_id,
                        &ann.source_bill.amendment_id,
                        &ann.source_bill.causative_text,
                        status,
                        ann.metadata.confidence,
                        &ann.metadata.annotator,
                        timestamp,
                        &ann.metadata.notes,
                        &ann.metadata.reasoning,
                    ])?;

                    let ann_id = tx.last_insert_rowid();
                    for path in &ann.paths {
                        path_stmt.execute(params![ann_id, path])?;
                    }
                }
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

    /// Collect element data into rows for batch insert (no recursion overhead per-insert)
    #[allow(clippy::type_complexity)]
    fn collect_element_rows<'a>(
        date: &'a str,
        element: &'a USLMElement,
        rows: &mut Vec<(&'a str, &'a str, String, Option<&'a str>, Option<&'a str>)>,
    ) {
        let element_type = format!("{:?}", element.data.element_type);
        let heading = element.data.heading.as_ref().map(|s| s.as_ref());
        let content = element.data.content.as_ref().map(|s| s.as_ref());
        rows.push((
            date,
            element.data.path.as_ref(),
            element_type,
            heading,
            content,
        ));

        for child in &element.children {
            Self::collect_element_rows(date, child, rows);
        }
    }

    fn index_element(
        stmt: &mut rusqlite::Statement,
        date: &str,
        element: &USLMElement,
    ) -> Result<(), DatasetError> {
        let mut rows = Vec::new();
        Self::collect_element_rows(date, element, &mut rows);
        for (d, path, elem_type, heading, content) in rows {
            stmt.execute(params![d, path, elem_type, heading, content])?;
        }
        Ok(())
    }

    /// Load a window of two versions with their annotations
    ///
    /// Returns InMemoryStorage with only the specified versions and their annotations.
    /// Bills, members, sponsors are NOT loaded - query them from storage as needed.
    pub fn load_window(&self, from: &str, to: &str) -> Result<InMemoryStorage, DatasetError> {
        let metadata = self.load_metadata()?;

        // Load only the two versions
        let from_v = self
            .get_version(from)?
            .ok_or_else(|| DatasetError::VersionNotFound(from.to_string()))?;
        let to_v = self
            .get_version(to)?
            .ok_or_else(|| DatasetError::VersionNotFound(to.to_string()))?;

        let mut versions = vec![from_v, to_v];
        versions.sort_by(|a, b| a.date.cmp(&b.date));

        // Load annotations for this pair only
        let mut diff_annotations = HashMap::new();
        if let Some(anns) = self.get_annotations(from, to)? {
            diff_annotations.insert((from.to_string(), to.to_string()), anns);
        }

        let mut storage = InMemoryStorage::from_parts(
            metadata,
            versions,
            HashMap::new(), // bills - query from storage
            diff_annotations,
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
        let versions = self.load_versions()?;
        let bills = self.load_bills()?;
        let members = self.load_members()?;
        let sponsors = self.load_sponsors()?;
        let diff_annotations = self.load_annotations()?;
        let bill_votes = self.load_bill_votes()?;

        let mut storage = InMemoryStorage::from_parts(
            metadata,
            versions,
            bills,
            diff_annotations,
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
        })
    }

    fn load_versions(&self) -> Result<Vec<VersionSnapshot>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT date, label, element_json FROM versions ORDER BY date")?;
        let mut rows = stmt.query([])?;

        let mut versions = Vec::new();
        while let Some(row) = rows.next()? {
            let date: String = row.get(0)?;
            let label: Option<String> = row.get(1)?;
            let element_json: String = row.get(2)?;
            let element: USLMElement = serde_json::from_str(&element_json)?;

            versions.push(VersionSnapshot {
                date,
                label,
                element,
            });
        }

        Ok(versions)
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

    fn load_annotations(
        &self,
    ) -> Result<HashMap<VersionPair, Vec<ChangeAnnotation>>, DatasetError> {
        use crate::annotation::{AnnotationMetadata, AnnotationStatus, BillReference};
        use crate::legislature::AmendingAction;
        use std::str::FromStr;

        // Load all annotations with their paths
        let mut stmt = self.conn.prepare(
            "SELECT a.id, a.from_date, a.to_date, a.operation, a.bill_id, a.amendment_id,
                    a.causative_text, a.status, a.confidence, a.annotator, a.timestamp,
                    a.notes, a.reasoning
             FROM annotations a
             ORDER BY a.from_date, a.to_date, a.id",
        )?;
        let mut rows = stmt.query([])?;

        let mut annotations: HashMap<VersionPair, Vec<ChangeAnnotation>> = HashMap::new();
        let mut ann_ids: Vec<(i64, String, String)> = Vec::new(); // (id, from, to)

        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let from_date: String = row.get(1)?;
            let to_date: String = row.get(2)?;
            let operation_str: String = row.get(3)?;
            let bill_id: String = row.get(4)?;
            let amendment_id: String = row.get(5)?;
            let causative_text: String = row.get(6)?;
            let status_str: String = row.get(7)?;
            let confidence: Option<f32> = row.get(8)?;
            let annotator: String = row.get(9)?;
            let timestamp_str: String = row.get(10)?;
            let notes: Option<String> = row.get(11)?;
            let reasoning: Option<String> = row.get(12)?;

            let operation =
                AmendingAction::from_str(&operation_str).unwrap_or(AmendingAction::Amend);
            let status = match status_str.as_str() {
                "Verified" => AnnotationStatus::Verified,
                "Disputed" => AnnotationStatus::Disputed,
                "Rejected" => AnnotationStatus::Rejected,
                _ => AnnotationStatus::Pending,
            };
            let timestamp = time::OffsetDateTime::parse(
                &timestamp_str,
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);

            let ann = ChangeAnnotation {
                operation,
                source_bill: BillReference {
                    bill_id,
                    amendment_id,
                    causative_text,
                },
                paths: Vec::new(), // Fill in below
                metadata: AnnotationMetadata {
                    status,
                    confidence,
                    annotator,
                    timestamp,
                    notes,
                    reasoning,
                },
            };

            let key = (from_date.clone(), to_date.clone());
            annotations.entry(key).or_default().push(ann);
            ann_ids.push((id, from_date, to_date));
        }

        // Load paths for each annotation
        let mut path_stmt = self
            .conn
            .prepare("SELECT path FROM annotation_paths WHERE annotation_id = ?1")?;

        for (id, from_date, to_date) in ann_ids {
            let mut path_rows = path_stmt.query(params![id])?;
            let mut paths = Vec::new();
            while let Some(row) = path_rows.next()? {
                let path: String = row.get(0)?;
                paths.push(path);
            }

            // Find the annotation and set paths
            let key = (from_date, to_date);
            if let Some(anns) = annotations.get_mut(&key) {
                // Find the annotation with matching id (it's the one with empty paths)
                for ann in anns.iter_mut() {
                    if ann.paths.is_empty() {
                        ann.paths = paths;
                        break;
                    }
                }
            }
        }

        Ok(annotations)
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

impl DocumentReader for SqliteStorage {
    fn list_versions(&self) -> Result<Vec<VersionInfo>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT date, label FROM versions ORDER BY date")?;
        let mut rows = stmt.query([])?;

        let mut versions = Vec::new();
        while let Some(row) = rows.next()? {
            let date: String = row.get(0)?;
            let label: Option<String> = row.get(1)?;
            versions.push(VersionInfo { date, label });
        }

        Ok(versions)
    }

    fn get_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT date, label, element_json FROM versions WHERE date = ?1")?;
        let mut rows = stmt.query(params![date])?;

        if let Some(row) = rows.next()? {
            let date: String = row.get(0)?;
            let label: Option<String> = row.get(1)?;
            let element_json: String = row.get(2)?;
            let element: USLMElement = serde_json::from_str(&element_json)?;
            Ok(Some(VersionSnapshot {
                date,
                label,
                element,
            }))
        } else {
            Ok(None)
        }
    }

    fn compute_diff(&self, from: &str, to: &str) -> Result<TreeDiff, DatasetError> {
        let from_v = self
            .get_version(from)?
            .ok_or_else(|| DatasetError::VersionNotFound(from.to_string()))?;
        let to_v = self
            .get_version(to)?
            .ok_or_else(|| DatasetError::VersionNotFound(to.to_string()))?;
        Ok(TreeDiff::from_elements(&from_v.element, &to_v.element))
    }

    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
        // Search using element_index table
        let query_pattern = format!("%{}%", query.to_lowercase());
        let mut stmt = self.conn.prepare(
            "SELECT version_date, path, 'heading', heading FROM element_index WHERE LOWER(heading) LIKE ?1
             UNION ALL
             SELECT version_date, path, 'content', content FROM element_index WHERE LOWER(content) LIKE ?1",
        )?;
        let mut rows = stmt.query(params![query_pattern])?;

        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            let date: String = row.get(0)?;
            let path: String = row.get(1)?;
            let field: String = row.get(2)?;
            let snippet: Option<String> = row.get(3)?;
            if let Some(snippet) = snippet {
                results.push(SearchResult {
                    date,
                    path,
                    field,
                    snippet,
                });
            }
        }

        Ok(results)
    }

    fn get_version_by_label(&self, label: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT date, label, element_json FROM versions WHERE label = ?1")?;
        let mut rows = stmt.query(params![label])?;

        if let Some(row) = rows.next()? {
            let date: String = row.get(0)?;
            let label: Option<String> = row.get(1)?;
            let element_json: String = row.get(2)?;
            let element: USLMElement = serde_json::from_str(&element_json)?;
            Ok(Some(VersionSnapshot {
                date,
                label,
                element,
            }))
        } else {
            Ok(None)
        }
    }

    fn next_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        let mut stmt = self.conn.prepare(
            "SELECT date, label, element_json FROM versions WHERE date > ?1 ORDER BY date LIMIT 1",
        )?;
        let mut rows = stmt.query(params![date])?;

        if let Some(row) = rows.next()? {
            let date: String = row.get(0)?;
            let label: Option<String> = row.get(1)?;
            let element_json: String = row.get(2)?;
            let element: USLMElement = serde_json::from_str(&element_json)?;
            Ok(Some(VersionSnapshot {
                date,
                label,
                element,
            }))
        } else {
            Ok(None)
        }
    }

    fn prev_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        let mut stmt = self.conn.prepare(
            "SELECT date, label, element_json FROM versions WHERE date < ?1 ORDER BY date DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![date])?;

        if let Some(row) = rows.next()? {
            let date: String = row.get(0)?;
            let label: Option<String> = row.get(1)?;
            let element_json: String = row.get(2)?;
            let element: USLMElement = serde_json::from_str(&element_json)?;
            Ok(Some(VersionSnapshot {
                date,
                label,
                element,
            }))
        } else {
            Ok(None)
        }
    }

    fn find_element(&self, path: &str) -> Result<Vec<(String, USLMElement)>, DatasetError> {
        // Use element_index to find which versions have this path
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT version_date FROM element_index WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;

        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            let date: String = row.get(0)?;
            if let Some(version) = self.get_version(&date)?
                && let Some(elem) = version.element.find(path)
            {
                results.push((date, elem.clone()));
            }
        }

        Ok(results)
    }
}

impl LinkReader for SqliteStorage {
    fn get_annotations(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<Vec<ChangeAnnotation>>, DatasetError> {
        let all = self.load_annotations()?;
        let key = (from.to_string(), to.to_string());
        Ok(all.get(&key).cloned())
    }

    fn annotations_for_path(&self, path: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        use crate::annotation::{AnnotationMetadata, AnnotationStatus, BillReference};
        use crate::legislature::AmendingAction;
        use std::str::FromStr;

        // Find annotation IDs that have this path
        let mut id_stmt = self
            .conn
            .prepare("SELECT DISTINCT annotation_id FROM annotation_paths WHERE path = ?1")?;
        let mut id_rows = id_stmt.query(params![path])?;

        let mut ann_ids: Vec<i64> = Vec::new();
        while let Some(row) = id_rows.next()? {
            ann_ids.push(row.get(0)?);
        }

        if ann_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Load those annotations
        let placeholders: String = ann_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, from_date, to_date, operation, bill_id, amendment_id, causative_text,
                    status, confidence, annotator, timestamp, notes, reasoning
             FROM annotations WHERE id IN ({})",
            placeholders
        );

        let mut stmt = self.conn.prepare(&query)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(ann_ids.iter()))?;

        let mut annotations = Vec::new();
        let mut loaded_ids = Vec::new();

        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let operation_str: String = row.get(3)?;
            let bill_id: String = row.get(4)?;
            let amendment_id: String = row.get(5)?;
            let causative_text: String = row.get(6)?;
            let status_str: String = row.get(7)?;
            let confidence: Option<f32> = row.get(8)?;
            let annotator: String = row.get(9)?;
            let timestamp_str: String = row.get(10)?;
            let notes: Option<String> = row.get(11)?;
            let reasoning: Option<String> = row.get(12)?;

            let operation =
                AmendingAction::from_str(&operation_str).unwrap_or(AmendingAction::Amend);
            let status = match status_str.as_str() {
                "Verified" => AnnotationStatus::Verified,
                "Disputed" => AnnotationStatus::Disputed,
                "Rejected" => AnnotationStatus::Rejected,
                _ => AnnotationStatus::Pending,
            };
            let timestamp = time::OffsetDateTime::parse(
                &timestamp_str,
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);

            annotations.push(ChangeAnnotation {
                operation,
                source_bill: BillReference {
                    bill_id,
                    amendment_id,
                    causative_text,
                },
                paths: Vec::new(),
                metadata: AnnotationMetadata {
                    status,
                    confidence,
                    annotator,
                    timestamp,
                    notes,
                    reasoning,
                },
            });
            loaded_ids.push(id);
        }

        // Load paths for each annotation
        let mut path_stmt = self
            .conn
            .prepare("SELECT path FROM annotation_paths WHERE annotation_id = ?1")?;

        for (ann, id) in annotations.iter_mut().zip(loaded_ids.iter()) {
            let mut path_rows = path_stmt.query(params![id])?;
            while let Some(row) = path_rows.next()? {
                ann.paths.push(row.get(0)?);
            }
        }

        Ok(annotations)
    }

    fn annotations_for_bill(&self, bill_id: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        use crate::annotation::{AnnotationMetadata, AnnotationStatus, BillReference};
        use crate::legislature::AmendingAction;
        use std::str::FromStr;

        let mut stmt = self.conn.prepare(
            "SELECT id, from_date, to_date, operation, bill_id, amendment_id, causative_text,
                    status, confidence, annotator, timestamp, notes, reasoning
             FROM annotations WHERE bill_id = ?1",
        )?;
        let mut rows = stmt.query(params![bill_id])?;

        let mut annotations = Vec::new();
        let mut loaded_ids = Vec::new();

        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            let operation_str: String = row.get(3)?;
            let bill_id: String = row.get(4)?;
            let amendment_id: String = row.get(5)?;
            let causative_text: String = row.get(6)?;
            let status_str: String = row.get(7)?;
            let confidence: Option<f32> = row.get(8)?;
            let annotator: String = row.get(9)?;
            let timestamp_str: String = row.get(10)?;
            let notes: Option<String> = row.get(11)?;
            let reasoning: Option<String> = row.get(12)?;

            let operation =
                AmendingAction::from_str(&operation_str).unwrap_or(AmendingAction::Amend);
            let status = match status_str.as_str() {
                "Verified" => AnnotationStatus::Verified,
                "Disputed" => AnnotationStatus::Disputed,
                "Rejected" => AnnotationStatus::Rejected,
                _ => AnnotationStatus::Pending,
            };
            let timestamp = time::OffsetDateTime::parse(
                &timestamp_str,
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);

            annotations.push(ChangeAnnotation {
                operation,
                source_bill: BillReference {
                    bill_id,
                    amendment_id,
                    causative_text,
                },
                paths: Vec::new(),
                metadata: AnnotationMetadata {
                    status,
                    confidence,
                    annotator,
                    timestamp,
                    notes,
                    reasoning,
                },
            });
            loaded_ids.push(id);
        }

        // Load paths
        let mut path_stmt = self
            .conn
            .prepare("SELECT path FROM annotation_paths WHERE annotation_id = ?1")?;

        for (ann, id) in annotations.iter_mut().zip(loaded_ids.iter()) {
            let mut path_rows = path_stmt.query(params![id])?;
            while let Some(row) = path_rows.next()? {
                ann.paths.push(row.get(0)?);
            }
        }

        Ok(annotations)
    }

    fn annotation_pairs(&self) -> Result<Vec<VersionPair>, DatasetError> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT from_date, to_date FROM annotations")?;
        let pairs = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(pairs)
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
}

impl DocumentWriter for SqliteStorage {
    fn metadata(&self) -> &DatasetMetadata {
        &self.metadata
    }

    fn set_metadata(&mut self, metadata: DatasetMetadata) {
        self.metadata = metadata;
        let _ = self.save_metadata();
    }

    fn add_version(&mut self, snapshot: VersionSnapshot) -> Result<(), DatasetError> {
        let element_json = serde_json::to_string(&snapshot.element)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO versions (date, label, element_json) VALUES (?1, ?2, ?3)",
            params![&snapshot.date, &snapshot.label, element_json],
        )?;

        // Index elements
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO element_index (version_date, path, element_type, heading, content) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        Self::index_element(&mut stmt, &snapshot.date, &snapshot.element)?;

        Ok(())
    }
}

impl LinkWriter for SqliteStorage {
    fn add_annotation(
        &mut self,
        from: &str,
        to: &str,
        annotation: ChangeAnnotation,
    ) -> Result<(), DatasetError> {
        let operation = format!("{:?}", annotation.operation);
        let status = format!("{:?}", annotation.metadata.status);
        let timestamp = annotation.metadata.timestamp.to_string();

        self.conn.execute(
            "INSERT INTO annotations (from_date, to_date, operation, bill_id, amendment_id, causative_text, status, confidence, annotator, timestamp, notes, reasoning) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                from,
                to,
                operation,
                &annotation.source_bill.bill_id,
                &annotation.source_bill.amendment_id,
                &annotation.source_bill.causative_text,
                status,
                annotation.metadata.confidence,
                &annotation.metadata.annotator,
                timestamp,
                &annotation.metadata.notes,
                &annotation.metadata.reasoning,
            ],
        )?;

        let ann_id = self.conn.last_insert_rowid();
        for path in &annotation.paths {
            self.conn.execute(
                "INSERT INTO annotation_paths (annotation_id, path) VALUES (?1, ?2)",
                params![ann_id, path],
            )?;
        }

        Ok(())
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
        // A database error here means we cannot show legislative material, so
        // the honest answer is that this dataset offers none.
        self.holds_legislature()
            .unwrap_or(false)
            .then_some(self as &dyn LegislatureReader)
    }
}
