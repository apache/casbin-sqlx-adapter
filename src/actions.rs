// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(clippy::suspicious_else_formatting)]
#![allow(clippy::toplevel_ref_arg)]
use crate::Error;
use casbin::{error::AdapterError, Error as CasbinError, Filter, Result};
use sqlx::error::Error as SqlxError;

use crate::models::{CasbinRule, NewCasbinRule};

#[cfg(feature = "postgres")]
pub type ConnectionPool = sqlx::PgPool;

#[cfg(feature = "mysql")]
pub type ConnectionPool = sqlx::MySqlPool;

#[cfg(feature = "sqlite")]
pub type ConnectionPool = sqlx::SqlitePool;

#[cfg(feature = "postgres")]
pub type QueryResult = sqlx::postgres::PgQueryResult;

#[cfg(feature = "mysql")]
pub type QueryResult = sqlx::mysql::MySqlQueryResult;

#[cfg(feature = "sqlite")]
pub type QueryResult = sqlx::sqlite::SqliteQueryResult;

// The statements below are run through the plain `sqlx::query` API on purpose:
// the `sqlx::query!` macros validate SQL against a live database at *compile*
// time, which would force every downstream crate to have a reachable database
// (or a matching `.sqlx` cache for its own driver) just to build this adapter.
#[cfg(feature = "postgres")]
mod sql {
    pub(super) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS casbin_rule (
                    id SERIAL PRIMARY KEY,
                    ptype VARCHAR NOT NULL,
                    v0 VARCHAR NOT NULL,
                    v1 VARCHAR NOT NULL,
                    v2 VARCHAR NOT NULL,
                    v3 VARCHAR NOT NULL,
                    v4 VARCHAR NOT NULL,
                    v5 VARCHAR NOT NULL,
                    CONSTRAINT unique_key_sqlx_adapter UNIQUE(ptype, v0, v1, v2, v3, v4, v5)
                    );";

    pub(super) const REMOVE_POLICY: &str = "DELETE FROM casbin_rule WHERE
                    ptype = $1 AND
                    v0 = $2 AND
                    v1 = $3 AND
                    v2 = $4 AND
                    v3 = $5 AND
                    v4 = $6 AND
                    v5 = $7";

    /// Indexed by `field_index`: entry `i` binds `ptype` plus `6 - i` values.
    pub(super) const REMOVE_FILTERED_POLICY: [&str; 6] = [
        "DELETE FROM casbin_rule WHERE
                    ptype = $1 AND
                    (v0 is NULL OR v0 = COALESCE($2,v0)) AND
                    (v1 is NULL OR v1 = COALESCE($3,v1)) AND
                    (v2 is NULL OR v2 = COALESCE($4,v2)) AND
                    (v3 is NULL OR v3 = COALESCE($5,v3)) AND
                    (v4 is NULL OR v4 = COALESCE($6,v4)) AND
                    (v5 is NULL OR v5 = COALESCE($7,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = $1 AND
                    (v1 is NULL OR v1 = COALESCE($2,v1)) AND
                    (v2 is NULL OR v2 = COALESCE($3,v2)) AND
                    (v3 is NULL OR v3 = COALESCE($4,v3)) AND
                    (v4 is NULL OR v4 = COALESCE($5,v4)) AND
                    (v5 is NULL OR v5 = COALESCE($6,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = $1 AND
                    (v2 is NULL OR v2 = COALESCE($2,v2)) AND
                    (v3 is NULL OR v3 = COALESCE($3,v3)) AND
                    (v4 is NULL OR v4 = COALESCE($4,v4)) AND
                    (v5 is NULL OR v5 = COALESCE($5,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = $1 AND
                    (v3 is NULL OR v3 = COALESCE($2,v3)) AND
                    (v4 is NULL OR v4 = COALESCE($3,v4)) AND
                    (v5 is NULL OR v5 = COALESCE($4,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = $1 AND
                    (v4 is NULL OR v4 = COALESCE($2,v4)) AND
                    (v5 is NULL OR v5 = COALESCE($3,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = $1 AND
                    (v5 is NULL OR v5 = COALESCE($2,v5))",
    ];

    pub(super) const LOAD_POLICY: &str =
        "SELECT id, ptype, v0, v1, v2, v3, v4, v5 FROM casbin_rule";

    pub(super) const LOAD_FILTERED_POLICY: &str =
        "SELECT id, ptype, v0, v1, v2, v3, v4, v5 FROM casbin_rule WHERE (
            ptype LIKE 'g%' AND v0 LIKE $1 AND v1 LIKE $2 AND v2 LIKE $3 AND v3 LIKE $4 AND v4 LIKE $5 AND v5 LIKE $6 )
        OR (
            ptype LIKE 'p%' AND v0 LIKE $7 AND v1 LIKE $8 AND v2 LIKE $9 AND v3 LIKE $10 AND v4 LIKE $11 AND v5 LIKE $12 )";

    pub(super) const ADD_POLICY: &str = "INSERT INTO casbin_rule ( ptype, v0, v1, v2, v3, v4, v5 )
                 VALUES ( $1, $2, $3, $4, $5, $6, $7 )";

    pub(super) const CLEAR_POLICY: &str = "DELETE FROM casbin_rule";
}

#[cfg(feature = "mysql")]
mod sql {
    pub(super) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS casbin_rule (
                    id INT NOT NULL AUTO_INCREMENT,
                    ptype VARCHAR(12) NOT NULL,
                    v0 VARCHAR(128) NOT NULL,
                    v1 VARCHAR(128) NOT NULL,
                    v2 VARCHAR(128) NOT NULL,
                    v3 VARCHAR(128) NOT NULL,
                    v4 VARCHAR(128) NOT NULL,
                    v5 VARCHAR(128) NOT NULL,
                    PRIMARY KEY(id),
                    CONSTRAINT unique_key_sqlx_adapter UNIQUE(ptype, v0, v1, v2, v3, v4, v5)
                ) ENGINE=InnoDB DEFAULT CHARSET=utf8;";

    pub(super) const REMOVE_POLICY: &str = "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    v0 = ? AND
                    v1 = ? AND
                    v2 = ? AND
                    v3 = ? AND
                    v4 = ? AND
                    v5 = ?";

    /// Indexed by `field_index`: entry `i` binds `ptype` plus `6 - i` values.
    pub(super) const REMOVE_FILTERED_POLICY: [&str; 6] = [
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v0 is NULL OR v0 = COALESCE(?,v0)) AND
                    (v1 is NULL OR v1 = COALESCE(?,v1)) AND
                    (v2 is NULL OR v2 = COALESCE(?,v2)) AND
                    (v3 is NULL OR v3 = COALESCE(?,v3)) AND
                    (v4 is NULL OR v4 = COALESCE(?,v4)) AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v1 is NULL OR v1 = COALESCE(?,v1)) AND
                    (v2 is NULL OR v2 = COALESCE(?,v2)) AND
                    (v3 is NULL OR v3 = COALESCE(?,v3)) AND
                    (v4 is NULL OR v4 = COALESCE(?,v4)) AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v2 is NULL OR v2 = COALESCE(?,v2)) AND
                    (v3 is NULL OR v3 = COALESCE(?,v3)) AND
                    (v4 is NULL OR v4 = COALESCE(?,v4)) AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v3 is NULL OR v3 = COALESCE(?,v3)) AND
                    (v4 is NULL OR v4 = COALESCE(?,v4)) AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v4 is NULL OR v4 = COALESCE(?,v4)) AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
    ];

    pub(super) const LOAD_POLICY: &str =
        "SELECT id, ptype, v0, v1, v2, v3, v4, v5 FROM casbin_rule";

    pub(super) const LOAD_FILTERED_POLICY: &str =
        "SELECT id, ptype, v0, v1, v2, v3, v4, v5 FROM casbin_rule WHERE (
            ptype LIKE 'g%' AND v0 LIKE ? AND v1 LIKE ? AND v2 LIKE ? AND v3 LIKE ? AND v4 LIKE ? AND v5 LIKE ? )
        OR (
            ptype LIKE 'p%' AND v0 LIKE ? AND v1 LIKE ? AND v2 LIKE ? AND v3 LIKE ? AND v4 LIKE ? AND v5 LIKE ? )";

    pub(super) const ADD_POLICY: &str = "INSERT INTO casbin_rule ( ptype, v0, v1, v2, v3, v4, v5 )
                 VALUES ( ?, ?, ?, ?, ?, ?, ? )";

    pub(super) const CLEAR_POLICY: &str = "DELETE FROM casbin_rule";
}

#[cfg(feature = "sqlite")]
mod sql {
    // `INTEGER PRIMARY KEY` is the only form SQLite treats as an auto-assigned
    // rowid alias; the `SERIAL PRIMARY KEY` this used to say left `id` NULL on
    // every insert, which then failed to decode back into `CasbinRule::id`.
    pub(super) const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS casbin_rule (
                    id INTEGER PRIMARY KEY,
                    ptype VARCHAR(12) NOT NULL,
                    v0 VARCHAR(128) NOT NULL,
                    v1 VARCHAR(128) NOT NULL,
                    v2 VARCHAR(128) NOT NULL,
                    v3 VARCHAR(128) NOT NULL,
                    v4 VARCHAR(128) NOT NULL,
                    v5 VARCHAR(128) NOT NULL,
                    CONSTRAINT unique_key_sqlx_adapter UNIQUE(ptype, v0, v1, v2, v3, v4, v5)
                    );";

    pub(super) const REMOVE_POLICY: &str = "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    v0 = ? AND
                    v1 = ? AND
                    v2 = ? AND
                    v3 = ? AND
                    v4 = ? AND
                    v5 = ?";

    /// Indexed by `field_index`: entry `i` binds `ptype` plus `6 - i` values.
    pub(super) const REMOVE_FILTERED_POLICY: [&str; 6] = [
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v0 is NULL OR v0 = COALESCE(?,v0)) AND
                    (v1 is NULL OR v1 = COALESCE(?,v1)) AND
                    (v2 is NULL OR v2 = COALESCE(?,v2)) AND
                    (v3 is NULL OR v3 = COALESCE(?,v3)) AND
                    (v4 is NULL OR v4 = COALESCE(?,v4)) AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v1 is NULL OR v1 = COALESCE(?,v1)) AND
                    (v2 is NULL OR v2 = COALESCE(?,v2)) AND
                    (v3 is NULL OR v3 = COALESCE(?,v3)) AND
                    (v4 is NULL OR v4 = COALESCE(?,v4)) AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v2 is NULL OR v2 = COALESCE(?,v2)) AND
                    (v3 is NULL OR v3 = COALESCE(?,v3)) AND
                    (v4 is NULL OR v4 = COALESCE(?,v4)) AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v3 is NULL OR v3 = COALESCE(?,v3)) AND
                    (v4 is NULL OR v4 = COALESCE(?,v4)) AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v4 is NULL OR v4 = COALESCE(?,v4)) AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
        "DELETE FROM casbin_rule WHERE
                    ptype = ? AND
                    (v5 is NULL OR v5 = COALESCE(?,v5))",
    ];

    pub(super) const LOAD_POLICY: &str =
        "SELECT id, ptype, v0, v1, v2, v3, v4, v5 FROM casbin_rule";

    pub(super) const LOAD_FILTERED_POLICY: &str =
        "SELECT id, ptype, v0, v1, v2, v3, v4, v5 FROM casbin_rule WHERE (
            ptype LIKE 'g%' AND v0 LIKE ? AND v1 LIKE ? AND v2 LIKE ? AND v3 LIKE ? AND v4 LIKE ? AND v5 LIKE ? )
        OR (
            ptype LIKE 'p%' AND v0 LIKE ? AND v1 LIKE ? AND v2 LIKE ? AND v3 LIKE ? AND v4 LIKE ? AND v5 LIKE ? )";

    pub(super) const ADD_POLICY: &str = "INSERT INTO casbin_rule ( ptype, v0, v1, v2, v3, v4, v5 )
                 VALUES ( ?, ?, ?, ?, ?, ?, ? )";

    pub(super) const CLEAR_POLICY: &str = "DELETE FROM casbin_rule";
}

fn adapter_error(err: SqlxError) -> CasbinError {
    CasbinError::from(AdapterError(Box::new(Error::SqlxError(err))))
}

pub async fn new(conn: &ConnectionPool) -> Result<QueryResult> {
    sqlx::query(sql::CREATE_TABLE)
        .execute(conn)
        .await
        .map_err(adapter_error)
}

pub async fn remove_policy(conn: &ConnectionPool, pt: &str, rule: Vec<String>) -> Result<bool> {
    let rule = normalize_casbin_rule(rule);
    sqlx::query(sql::REMOVE_POLICY)
        .bind(pt)
        .bind(rule[0].as_str())
        .bind(rule[1].as_str())
        .bind(rule[2].as_str())
        .bind(rule[3].as_str())
        .bind(rule[4].as_str())
        .bind(rule[5].as_str())
        .execute(conn)
        .await
        .map(|n| n.rows_affected() == 1)
        .map_err(adapter_error)
}

pub async fn remove_policies(
    conn: &ConnectionPool,
    pt: &str,
    rules: Vec<Vec<String>>,
) -> Result<bool> {
    let mut transaction = conn.begin().await.map_err(adapter_error)?;
    for rule in rules {
        let rule = normalize_casbin_rule(rule);
        sqlx::query(sql::REMOVE_POLICY)
            .bind(pt)
            .bind(rule[0].as_str())
            .bind(rule[1].as_str())
            .bind(rule[2].as_str())
            .bind(rule[3].as_str())
            .bind(rule[4].as_str())
            .bind(rule[5].as_str())
            .execute(&mut *transaction)
            .await
            .and_then(|n| {
                if n.rows_affected() == 1 {
                    Ok(true)
                } else {
                    Err(SqlxError::RowNotFound)
                }
            })
            .map_err(adapter_error)?;
    }
    transaction.commit().await.map_err(adapter_error)?;
    Ok(true)
}

pub async fn remove_filtered_policy(
    conn: &ConnectionPool,
    pt: &str,
    field_index: usize,
    field_values: Vec<String>,
) -> Result<bool> {
    let field_values = normalize_casbin_rule_option(field_values);
    let field_index = if field_index > 5 { 0 } else { field_index };

    let mut query = sqlx::query(sql::REMOVE_FILTERED_POLICY[field_index]).bind(pt);
    for value in field_values.iter().take(6 - field_index) {
        query = query.bind(value.as_deref());
    }

    query
        .execute(conn)
        .await
        .map(|n| n.rows_affected() >= 1)
        .map_err(adapter_error)
}

pub(crate) async fn load_policy(conn: &ConnectionPool) -> Result<Vec<CasbinRule>> {
    let casbin_rule: Vec<CasbinRule> = sqlx::query_as::<_, CasbinRule>(sql::LOAD_POLICY)
        .fetch_all(conn)
        .await
        .map_err(adapter_error)?;

    Ok(casbin_rule)
}

pub(crate) async fn load_filtered_policy(
    conn: &ConnectionPool,
    filter: &Filter<'_>,
) -> Result<Vec<CasbinRule>> {
    let (g_filter, p_filter) = filtered_where_values(filter);

    let mut query = sqlx::query_as::<_, CasbinRule>(sql::LOAD_FILTERED_POLICY);
    for value in g_filter.iter().chain(p_filter.iter()) {
        query = query.bind(*value);
    }

    let casbin_rule: Vec<CasbinRule> = query.fetch_all(conn).await.map_err(adapter_error)?;

    Ok(casbin_rule)
}

fn filtered_where_values<'a>(filter: &Filter<'a>) -> ([&'a str; 6], [&'a str; 6]) {
    let mut g_filter: [&'a str; 6] = ["%", "%", "%", "%", "%", "%"];
    let mut p_filter: [&'a str; 6] = ["%", "%", "%", "%", "%", "%"];
    for (idx, val) in filter.g.iter().enumerate() {
        if val != &"" {
            g_filter[idx] = val;
        }
    }
    for (idx, val) in filter.p.iter().enumerate() {
        if val != &"" {
            p_filter[idx] = val;
        }
    }
    (g_filter, p_filter)
}

pub(crate) async fn save_policy(
    conn: &ConnectionPool,
    rules: Vec<NewCasbinRule<'_>>,
) -> Result<()> {
    let mut transaction = conn.begin().await.map_err(adapter_error)?;
    sqlx::query(sql::CLEAR_POLICY)
        .execute(&mut *transaction)
        .await
        .map_err(adapter_error)?;
    for rule in rules {
        sqlx::query(sql::ADD_POLICY)
            .bind(rule.ptype)
            .bind(rule.v0)
            .bind(rule.v1)
            .bind(rule.v2)
            .bind(rule.v3)
            .bind(rule.v4)
            .bind(rule.v5)
            .execute(&mut *transaction)
            .await
            .and_then(|n| {
                if n.rows_affected() == 1 {
                    Ok(true)
                } else {
                    Err(SqlxError::RowNotFound)
                }
            })
            .map_err(adapter_error)?;
    }
    transaction.commit().await.map_err(adapter_error)?;
    Ok(())
}

pub(crate) async fn add_policy(conn: &ConnectionPool, rule: NewCasbinRule<'_>) -> Result<bool> {
    sqlx::query(sql::ADD_POLICY)
        .bind(rule.ptype)
        .bind(rule.v0)
        .bind(rule.v1)
        .bind(rule.v2)
        .bind(rule.v3)
        .bind(rule.v4)
        .bind(rule.v5)
        .execute(conn)
        .await
        .map(|n| n.rows_affected() == 1)
        .map_err(adapter_error)?;

    Ok(true)
}

pub(crate) async fn add_policies(
    conn: &ConnectionPool,
    rules: Vec<NewCasbinRule<'_>>,
) -> Result<bool> {
    let mut transaction = conn.begin().await.map_err(adapter_error)?;
    for rule in rules {
        sqlx::query(sql::ADD_POLICY)
            .bind(rule.ptype)
            .bind(rule.v0)
            .bind(rule.v1)
            .bind(rule.v2)
            .bind(rule.v3)
            .bind(rule.v4)
            .bind(rule.v5)
            .execute(&mut *transaction)
            .await
            .and_then(|n| {
                if n.rows_affected() == 1 {
                    Ok(true)
                } else {
                    Err(SqlxError::RowNotFound)
                }
            })
            .map_err(adapter_error)?;
    }
    transaction.commit().await.map_err(adapter_error)?;
    Ok(true)
}

pub(crate) async fn clear_policy(conn: &ConnectionPool) -> Result<()> {
    let mut transaction = conn.begin().await.map_err(adapter_error)?;
    sqlx::query(sql::CLEAR_POLICY)
        .execute(&mut *transaction)
        .await
        .map_err(adapter_error)?;
    transaction.commit().await.map_err(adapter_error)?;
    Ok(())
}

fn normalize_casbin_rule(mut rule: Vec<String>) -> Vec<String> {
    rule.resize(6, String::new());
    rule
}

fn normalize_casbin_rule_option(rule: Vec<String>) -> Vec<Option<String>> {
    let mut rule_with_option = rule
        .iter()
        .map(|x| match x.is_empty() {
            true => None,
            false => Some(x.clone()),
        })
        .collect::<Vec<Option<String>>>();
    rule_with_option.resize(6, None);
    rule_with_option
}
