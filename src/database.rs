// Furtherance - Track your time without being tracked
// Copyright (C) 2024  Ricky Kresslein <rk@unobserved.io>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use rusqlite::{backup, params, Connection, Result};
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use crate::models::{
    fur_settings::FurSettings, fur_shortcut::FurShortcut, fur_task::FurTask,
    group_to_edit::GroupToEdit,
};

#[derive(Debug)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for SortOrder {
    fn default() -> Self {
        Self::Ascending
    }
}

impl SortOrder {
    fn to_sqlite(&self) -> &str {
        match self {
            SortOrder::Ascending => "ASC",
            SortOrder::Descending => "DESC",
        }
    }
}

#[derive(Debug)]
pub enum SortBy {
    StartTime,
    StopTime,
    TaskName,
}

impl Default for SortBy {
    fn default() -> Self {
        Self::StartTime
    }
}

impl SortBy {
    fn to_sqlite(&self) -> &str {
        match self {
            Self::StartTime => "start_time",
            Self::StopTime => "stop_time",
            Self::TaskName => "task_name",
        }
    }
}

pub fn db_get_directory() -> PathBuf {
    // Get DB location from settings
    let settings_db_dir = match FurSettings::new() {
        Ok(loaded_settings) => loaded_settings.database_url,
        Err(e) => {
            eprintln!("Error loading settings: {}", e);
            FurSettings::default().database_url
        }
    };

    PathBuf::from(&settings_db_dir)
}

pub fn db_init() -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tasks (
                        id INTEGER PRIMARY KEY,
                        task_name TEXT,
                        start_time TIMESTAMP,
                        stop_time TIMESTAMP,
                        tags TEXT,
                        project TEXT,
                        rate REAL,
                        currency TEXT);",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS shortcuts (
                        id INTEGER PRIMARY KEY,
                        name TEXT,
                        tags TEXT,
                        project TEXT,
                        rate REAL,
                        currency TEXT,
                        color_hex TEXT);",
        [],
    )?;

    Ok(())
}

pub fn db_upgrade_old() -> Result<()> {
    // Update from old DB w/o tags, project, or rates
    let conn = Connection::open(db_get_directory())?;

    let _ = db_add_tags_column(&conn);
    let _ = db_add_project_column(&conn);
    let _ = db_add_rate_column(&conn);
    let _ = db_add_currency_column(&conn);

    Ok(())
}

pub fn db_add_tags_column(conn: &Connection) -> Result<()> {
    conn.execute("ALTER TABLE tasks ADD COLUMN tags TEXT DEFAULT ''", [])?;
    Ok(())
}

pub fn db_add_project_column(conn: &Connection) -> Result<()> {
    conn.execute("ALTER TABLE tasks ADD COLUMN project TEXT DEFAULT ''", [])?;
    Ok(())
}

pub fn db_add_rate_column(conn: &Connection) -> Result<()> {
    conn.execute("ALTER TABLE tasks ADD COLUMN rate REAL DEFAULT 0.0", [])?;
    Ok(())
}

pub fn db_add_currency_column(conn: &Connection) -> Result<()> {
    conn.execute("ALTER TABLE tasks ADD COLUMN currency Text DEFAULT ''", [])?;
    Ok(())
}

pub fn db_write_task(fur_task: FurTask) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute(
        "INSERT INTO tasks (
            task_name,
            start_time,
            stop_time,
            tags,
            project,
            rate
        ) values (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            fur_task.name,
            fur_task.start_time.to_rfc3339(),
            fur_task.stop_time.to_rfc3339(),
            fur_task.tags,
            fur_task.project,
            fur_task.rate,
        ],
    )?;

    Ok(())
}

pub fn db_write_tasks(tasks: &[FurTask]) -> Result<()> {
    let mut conn = Connection::open(db_get_directory())?;

    let tx = conn.transaction()?;

    {
        let mut stmt = tx.prepare(
            "
            INSERT INTO tasks (task_name, start_time, stop_time, tags, project, rate, currency)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        )?;

        for task in tasks {
            stmt.execute(params![
                task.name,
                task.start_time.to_rfc3339(),
                task.stop_time.to_rfc3339(),
                task.tags,
                task.project,
                task.rate,
                task.currency,
            ])?;
        }
    } // stmt is dropped here, releasing the borrow on tx

    tx.commit()?;

    Ok(())
}

pub fn db_retrieve_all_tasks(
    sort: SortBy,
    order: SortOrder,
) -> Result<Vec<FurTask>, rusqlite::Error> {
    // Retrieve all tasks from the database
    let conn = Connection::open(db_get_directory())?;

    let mut stmt = conn.prepare(
        format!(
            "SELECT * FROM tasks ORDER BY {0} {1}",
            sort.to_sqlite(),
            order.to_sqlite()
        )
        .as_str(),
    )?;
    let mut rows = stmt.query(params![])?;

    let mut tasks_vec: Vec<FurTask> = Vec::new();

    while let Some(row) = rows.next()? {
        let fur_task = FurTask {
            id: row.get(0)?,
            name: row.get(1)?,
            start_time: row.get(2)?,
            stop_time: row.get(3)?,
            tags: row.get(4)?,
            project: row.get(5)?,
            rate: row.get(6)?,
            currency: String::new(),
        };
        tasks_vec.push(fur_task);
    }

    Ok(tasks_vec)
}

pub fn db_retrieve_tasks_by_date_range(
    start_date: String,
    end_date: String,
) -> Result<Vec<FurTask>> {
    let conn = Connection::open(db_get_directory())?;
    let mut stmt = conn.prepare(
        "SELECT * FROM tasks WHERE start_time BETWEEN ?1 AND ?2 ORDER BY start_time ASC",
    )?;
    let mut rows = stmt.query(params![start_date, end_date])?;

    let mut tasks_vec: Vec<FurTask> = Vec::new();

    while let Some(row) = rows.next()? {
        let fur_task = FurTask {
            id: row.get(0)?,
            name: row.get(1)?,
            start_time: row.get(2)?,
            stop_time: row.get(3)?,
            tags: row.get(4)?,
            project: row.get(5)?,
            rate: row.get(6)?,
            currency: String::new(),
        };
        tasks_vec.push(fur_task);
    }

    Ok(tasks_vec)
}

/// Retrieve a limited number of days worth of tasks
pub fn db_retrieve_tasks_with_day_limit(
    days: i64,
    sort: SortBy,
    order: SortOrder,
) -> Result<Vec<FurTask>> {
    let conn = Connection::open(db_get_directory())?;

    // Construct the query string dynamically
    let query = format!(
        "SELECT * FROM tasks WHERE start_time >= date('now', ?) ORDER BY {} {}",
        sort.to_sqlite(),
        order.to_sqlite()
    );

    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query(params![format!("-{} days", days - 1)])?;

    let mut tasks_vec: Vec<FurTask> = Vec::new();

    while let Some(row) = rows.next()? {
        let fur_task = FurTask {
            id: row.get(0)?,
            name: row.get(1)?,
            start_time: row.get(2)?,
            stop_time: row.get(3)?,
            tags: row.get(4)?,
            project: row.get(5)?,
            rate: row.get(6)?,
            currency: String::new(),
        };
        tasks_vec.push(fur_task);
    }

    Ok(tasks_vec)
}

pub fn db_update_task(fur_task: FurTask) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute(
        "UPDATE tasks SET
            task_name = ?1,
            start_time = ?2,
            stop_time = ?3,
            tags = ?4,
            project = ?5,
            rate = ?6
        WHERE id = ?7",
        params![
            fur_task.name,
            fur_task.start_time.to_rfc3339(),
            fur_task.stop_time.to_rfc3339(),
            fur_task.tags,
            fur_task.project,
            fur_task.rate,
            fur_task.id,
        ],
    )?;

    Ok(())
}

pub fn db_update_group_of_tasks(group: &GroupToEdit) -> Result<()> {
    let mut conn = Connection::open(db_get_directory())?;
    // Transaction ensures all updates succeed or none do.
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            "UPDATE tasks SET
            task_name = ?1,
            tags = ?2,
            project = ?3,
            rate = ?4
        WHERE id = ?5",
        )?;

        for id in group.task_ids().iter() {
            stmt.execute(params![
                group.new_name.trim(),
                group
                    .new_tags
                    .trim()
                    .strip_prefix('#')
                    .unwrap_or(&group.tags)
                    .trim()
                    .to_string(),
                group.new_project.trim(),
                group.new_rate.trim().parse::<f32>().unwrap_or(0.0),
                id,
            ])?;
        }
    }

    // Commit the transaction
    tx.commit()?;

    Ok(())
}

pub fn update_start_time(id: i32, start_time: String) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute(
        "UPDATE tasks SET start_time = (?1) WHERE id = (?2)",
        params![start_time, id],
    )?;

    Ok(())
}

pub fn update_stop_time(id: i32, stop_time: String) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute(
        "UPDATE tasks SET stop_time = (?1) WHERE id = (?2)",
        params![stop_time, id],
    )?;

    Ok(())
}

pub fn update_task_name(id: i32, task_name: String) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute(
        "UPDATE tasks SET task_name = (?1) WHERE id = (?2)",
        params![task_name, id],
    )?;

    Ok(())
}

pub fn update_tags(id: i32, tags: String) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute(
        "UPDATE tasks SET tags = (?1) WHERE id = (?2)",
        params![tags, id],
    )?;

    Ok(())
}

pub fn update_project(id: i32, project: String) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute(
        "UPDATE tasks SET project = (?1) WHERE id = (?2)",
        params![project, id],
    )?;

    Ok(())
}

pub fn update_rate(id: i32, rate: f32) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute(
        "UPDATE tasks SET rate = (?1) WHERE id = (?2)",
        params![rate, id],
    )?;

    Ok(())
}

pub fn get_tasks_by_id(id_list: Vec<i32>) -> Result<Vec<FurTask>, rusqlite::Error> {
    let conn = Connection::open(db_get_directory())?;
    let mut stmt = conn.prepare("SELECT * FROM tasks WHERE id = ?")?;
    let mut tasks_vec = Vec::new();

    for id in id_list {
        let task_iter = stmt.query_map(&[&id], |row| {
            Ok(FurTask {
                id: row.get(0)?,
                name: row.get(1)?,
                start_time: row.get(2)?,
                stop_time: row.get(3)?,
                tags: row.get(4)?,
                project: row.get(5)?,
                rate: row.get(6)?,
                currency: String::new(),
            })
        })?;

        for task_item in task_iter {
            tasks_vec.push(task_item?);
        }
    }

    Ok(tasks_vec)
}

pub fn get_tasks_by_name_and_tags(
    task_name: String,
    tag_list: Vec<String>,
) -> Result<Vec<FurTask>, rusqlite::Error> {
    let conn = Connection::open(db_get_directory())?;

    let name_param = format!("%{}%", task_name);
    let tag_list_params: Vec<String> = tag_list.iter().map(|tag| format!("%{}%", tag)).collect();

    let mut sql_query = String::from("SELECT * FROM tasks WHERE lower(task_name) LIKE lower(?)");
    tag_list_params
        .iter()
        .for_each(|_| sql_query.push_str(" AND lower(tags) LIKE lower(?)"));
    sql_query.push_str(" ORDER BY task_name");

    let mut query = conn.prepare(sql_query.as_str())?;
    query.raw_bind_parameter(1, name_param)?;
    for (i, tag) in tag_list_params.iter().enumerate() {
        query.raw_bind_parameter(i + 2, tag)?;
    }

    let tasks_vec = query
        .raw_query()
        .mapped(|row| {
            Ok(FurTask {
                id: row.get(0)?,
                name: row.get(1)?,
                start_time: row.get(2)?,
                stop_time: row.get(3)?,
                tags: row.get(4)?,
                project: row.get(5)?,
                rate: row.get(6)?,
                currency: String::new(),
            })
        })
        .map(|task_item| task_item.unwrap())
        .collect();

    Ok(tasks_vec)
}

pub fn check_for_tasks() -> Result<String> {
    let conn = Connection::open(db_get_directory())?;

    conn.query_row(
        "SELECT task_name FROM tasks ORDER BY ROWID ASC LIMIT 1",
        [],
        |row| row.get(0),
    )
}

pub fn db_task_exists(task: &FurTask) -> Result<bool> {
    let conn = Connection::open(db_get_directory())?;

    let query = "
        SELECT 1 FROM tasks
        WHERE task_name = ?1
        AND start_time = ?2
        AND stop_time = ?3
        AND tags = ?4
        AND project = ?5
        AND rate = ?6
        AND currency = ?7
        LIMIT 1
    ";

    let mut stmt = conn.prepare(query)?;

    let exists = stmt.exists(params![
        task.name,
        task.start_time.to_rfc3339(),
        task.stop_time.to_rfc3339(),
        task.tags,
        task.project,
        task.rate,
        task.currency,
    ])?;

    Ok(exists)
}

pub fn db_delete_tasks_by_ids(id_list: Vec<u32>) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    for id in id_list {
        conn.execute("delete FROM tasks WHERE id = (?1)", &[&id.to_string()])?;
    }

    Ok(())
}

/// Write a shortcut to the database
pub fn db_write_shortcut(fur_shortcut: FurShortcut) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;
    conn.execute(
        "INSERT INTO shortcuts (
            name,
            tags,
            project,
            rate,
            currency,
            color_hex
        ) values (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            fur_shortcut.name,
            fur_shortcut.tags,
            fur_shortcut.project,
            fur_shortcut.rate,
            fur_shortcut.currency,
            fur_shortcut.color_hex,
        ],
    )?;

    Ok(())
}

/// Retrieve all shortcuts from the database
pub fn db_retrieve_shortcuts() -> Result<Vec<FurShortcut>, rusqlite::Error> {
    let conn = Connection::open(db_get_directory())?;

    let mut stmt = conn.prepare("SELECT * FROM shortcuts ORDER BY name")?;
    let mut rows = stmt.query(params![])?;

    let mut shortcuts: Vec<FurShortcut> = Vec::new();

    while let Some(row) = rows.next()? {
        let fur_shortcut = FurShortcut {
            id: row.get(0)?,
            name: row.get(1)?,
            tags: row.get(2)?,
            project: row.get(3)?,
            rate: row.get(4)?,
            currency: row.get(5)?,
            color_hex: row.get(6)?,
        };
        shortcuts.push(fur_shortcut);
    }

    Ok(shortcuts)
}

pub fn db_update_shortcut(fur_shortcut: FurShortcut) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute(
        "UPDATE shortcuts SET
            name = (?1),
            tags = (?2),
            project = (?3),
            rate = (?4),
            currency = (?5),
            color_hex = (?6)
        WHERE id = (?7)",
        params![
            fur_shortcut.name,
            fur_shortcut.tags,
            fur_shortcut.project,
            fur_shortcut.rate,
            fur_shortcut.currency,
            fur_shortcut.color_hex,
            fur_shortcut.id,
        ],
    )?;

    Ok(())
}

pub fn db_delete_shortcut_by_id(id: u32) -> Result<()> {
    let conn = Connection::open(db_get_directory())?;

    conn.execute("delete FROM shortcuts WHERE id = (?1)", &[&id.to_string()])?;

    Ok(())
}

pub fn delete_all() -> Result<()> {
    // Delete everything from the database
    let conn = Connection::open(db_get_directory())?;

    conn.execute("delete from tasks", [])?;

    Ok(())
}

pub fn db_backup(backup_file: PathBuf) -> Result<()> {
    let mut bkup_conn = Connection::open(backup_file)?;
    let conn = Connection::open(db_get_directory())?;
    let backup = backup::Backup::new(&conn, &mut bkup_conn)?;
    backup.run_to_completion(5, Duration::from_millis(250), None)
}

// pub fn import_db(new_db: String) -> Result<()> {
//     let new_conn = Connection::open(new_db.clone())?;
//     let valid = match check_db_validity(new_db) {
//         Ok(_) => true,
//         Err(_) => false,
//     };

//     if valid {
//         let mut conn = Connection::open(db_get_directory())?;
//         let backup = backup::Backup::new(&new_conn, &mut conn)?;
//         backup.run_to_completion(5, Duration::from_millis(250), None)
//     } else {
//         // TODO: Show error
//         Ok(())
//     }
// }

pub fn db_is_valid_v3(path: &Path) -> Result<bool> {
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(_) => return Ok(false),
    };

    // Check if the table 'tasks' exists
    let mut stmt =
        match conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='tasks'") {
            Ok(stmt) => stmt,
            Err(_) => return Ok(false),
        };
    let table_exists = match stmt.exists([]) {
        Ok(exists) => exists,
        Err(_) => return Ok(false),
    };
    if !table_exists {
        return Ok(false);
    }

    // Verify the table's structure
    let expected_columns = [
        "id integer",
        "task_name text",
        "start_time timestamp",
        "stop_time timestamp",
        "tags text",
        "project text",
        "rate real",
        "currency text",
    ];
    let mut stmt = match conn.prepare("PRAGMA table_info(tasks)") {
        Ok(stmt) => stmt,
        Err(_) => return Ok(false),
    };
    let column_info = match stmt.query_map([], |row| {
        Ok(format!(
            "{} {}",
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?.to_lowercase()
        ))
    }) {
        Ok(iter) => iter,
        Err(_) => return Ok(false),
    };

    let mut columns: Vec<String> = Vec::new();
    for column in column_info {
        match column {
            Ok(col) => columns.push(col),
            Err(_) => return Ok(false),
        }
    }
    for expected_col in expected_columns.iter() {
        if !columns.contains(&expected_col.to_string()) {
            return Ok(false);
        }
    }

    Ok(true)
}

pub fn db_is_valid_v1(path: &Path) -> Result<bool> {
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(_) => return Ok(false),
    };

    // Check if the table 'tasks' exists
    let mut stmt =
        match conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='tasks'") {
            Ok(stmt) => stmt,
            Err(_) => return Ok(false),
        };
    let table_exists = match stmt.exists([]) {
        Ok(exists) => exists,
        Err(_) => return Ok(false),
    };
    if !table_exists {
        return Ok(false);
    }

    // Verify the table's structure
    let expected_columns = [
        "id integer",
        "task_name text",
        "start_time timestamp",
        "stop_time timestamp",
        "tags text",
    ];
    let mut stmt = match conn.prepare("PRAGMA table_info(tasks)") {
        Ok(stmt) => stmt,
        Err(_) => return Ok(false),
    };
    let column_info = match stmt.query_map([], |row| {
        Ok(format!(
            "{} {}",
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?.to_lowercase()
        ))
    }) {
        Ok(iter) => iter,
        Err(_) => return Ok(false),
    };

    let mut columns: Vec<String> = Vec::new();
    for column in column_info {
        match column {
            Ok(col) => columns.push(col),
            Err(_) => return Ok(false),
        }
    }
    for expected_col in expected_columns.iter() {
        if !columns.contains(&expected_col.to_string()) {
            return Ok(false);
        }
    }

    Ok(true)
}
