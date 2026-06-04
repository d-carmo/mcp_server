use anyhow::{Context, Result};
use mysql::prelude::*;
use mysql::{params, Pool, Row};
use serde_json::{json, Map, Value};

use super::{BoxFuture, McpTool, ToolRegistration};

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

pub struct QueryDBTool;

impl McpTool for QueryDBTool {
    fn name(&self) -> &'static str {
        "query_db"
    }

    fn description(&self) -> &'static str {
        "Execute a SELECT query against the database scoped to a specific user. \
         The query MUST be a SELECT statement and MUST include the named parameter \
         :user_id (e.g. WHERE user_id = :user_id) so results are always scoped to \
         that user — never the full table."
    }

    fn schema(&self) -> Map<String, Value> {
        json!({
            "type": "object",
            "properties": {
                "user_id": {
                    "type": "string",
                    "description": "The user ID whose data to query. Bound to :user_id in the query."
                },
                "query": {
                    "type": "string",
                    "description": "A SELECT-only SQL query. Must include :user_id as a named parameter \
                                    to scope results (e.g. WHERE user_id = :user_id)."
                }
            },
            "required": ["user_id", "query"]
        })
        .as_object()
        .expect("schema literal is a valid JSON object")
        .clone()
    }

    fn call(&self, params: Map<String, Value>) -> BoxFuture<'_, Result<String>> {
        Box::pin(async move {
            let user_id = params
                .get("user_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            let query = params
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();

            if user_id.is_empty() {
                return Ok("Error: user_id is required".to_string());
            }
            if query.is_empty() {
                return Ok("Error: query is required".to_string());
            }

            if let Err(msg) = validate_select_query(&query) {
                return Ok(format!("Error: {}", msg));
            }

            tokio::task::spawn_blocking(move || execute_query(&query, &user_id))
                .await
                .map_err(|e| anyhow::anyhow!("query task panicked: {}", e))?
        })
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Returns `Err` with a human-readable message if the query is not a safe,
/// user-scoped SELECT.
fn validate_select_query(query: &str) -> std::result::Result<(), String> {
    let trimmed = query.trim();

    if !trimmed.to_ascii_uppercase().starts_with("SELECT") {
        return Err("only SELECT queries are allowed".to_string());
    }

    // Block statement stacking (e.g. "SELECT 1; DROP TABLE users")
    let body = trimmed.trim_end_matches(';');
    if body.contains(';') {
        return Err("multiple statements are not allowed".to_string());
    } else {
        // checking there's a single :user_id filtered (avoiding user_id = X OR user_id = y)
        let count = body.matches(":user_id").count();
        if count > 1 {
            return Err("query must include only one :user_id parameter to ensure proper scoping".to_string());
        }   
    }
    


    // Require explicit user scoping via named parameter
    if !trimmed.contains(":user_id") {
        return Err(
            "query must include :user_id as a named parameter to scope results to the user \
             (e.g. WHERE user_id = :user_id)"
                .to_string(),
        );
    }

    Ok(())
}

fn execute_query(query: &str, user_id: &str) -> Result<String> {
    let host = std::env::var("DB_URL").context("DB_URL not set")?;
    let name = std::env::var("DB_NAME").context("DB_NAME not set")?;
    let user = std::env::var("DB_USER").context("DB_USER not set")?;
    let pass = std::env::var("DB_PASSWORD").unwrap_or_default();

    let url = format!("mysql://{}:{}@{}/{}", user, pass, host, name);
    let pool = Pool::new(url.as_str()).context("failed to create DB pool")?;
    let mut conn = pool.get_conn().context("failed to connect to DB")?;

    let rows: Vec<Row> = conn
        .exec(query, params! { "user_id" => user_id })
        .context("query execution failed")?;

    let result: Vec<Map<String, Value>> = rows
        .iter()
        .map(|row| {
            row.columns_ref()
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    let col_name = col.name_str().into_owned();
                    let col_value = mysql_value_to_json(row[i].clone());
                    (col_name, col_value)
                })
                .collect()
        })
        .collect();

    serde_json::to_string_pretty(&result).context("failed to serialize result")
}

fn mysql_value_to_json(v: mysql::Value) -> Value {
    match v {
        mysql::Value::NULL => Value::Null,
        mysql::Value::Bytes(b) => String::from_utf8(b)
            .map(Value::String)
            .unwrap_or_else(|_| Value::String("<binary data>".to_string())),
        mysql::Value::Int(i) => Value::Number(i.into()),
        mysql::Value::UInt(u) => Value::Number(u.into()),
        mysql::Value::Float(f) => serde_json::Number::from_f64(f as f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        mysql::Value::Double(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        mysql::Value::Date(y, mo, d, h, min, s, us) => Value::String(format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:06}",
            y, mo, d, h, min, s, us
        )),
        mysql::Value::Time(neg, d, h, m, s, us) => {
            let prefix = if neg { "-" } else { "" };
            Value::String(format!(
                "{}{}:{:02}:{:02}.{:06}",
                prefix,
                d * 24 + h as u32,
                m,
                s,
                us
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Inventory registration — picked up automatically at link time
// ---------------------------------------------------------------------------

inventory::submit! { ToolRegistration { factory: || Box::new(QueryDBTool) } }

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_select_query ---

    #[test]
    fn rejects_non_select() {
        assert!(validate_select_query("DELETE FROM users WHERE user_id = :user_id").is_err());
        assert!(validate_select_query("INSERT INTO t VALUES (1)").is_err());
        assert!(validate_select_query("UPDATE t SET x=1 WHERE user_id = :user_id").is_err());
        assert!(validate_select_query("DROP TABLE t").is_err());
    }

    #[test]
    fn rejects_multiple_statements() {
        assert!(validate_select_query(
            "SELECT * FROM t WHERE user_id = :user_id; DROP TABLE t"
        )
        .is_err());
    }

    #[test]
    fn rejects_missing_user_id_param() {
        assert!(validate_select_query("SELECT * FROM t WHERE id = 1").is_err());
    }

    #[test]
    fn accepts_valid_query() {
        assert!(validate_select_query(
            "SELECT id, name FROM users WHERE user_id = :user_id"
        )
        .is_ok());
    }

    #[test]
    fn accepts_query_with_trailing_semicolon() {
        assert!(validate_select_query(
            "SELECT id FROM orders WHERE user_id = :user_id;"
        )
        .is_ok());
    }

    #[test]
    fn accepts_case_insensitive_select() {
        assert!(validate_select_query(
            "select id from t where user_id = :user_id"
        )
        .is_ok());
    }

    // --- mysql_value_to_json ---

    #[test]
    fn null_maps_to_json_null() {
        assert_eq!(mysql_value_to_json(mysql::Value::NULL), Value::Null);
    }

    #[test]
    fn int_maps_to_json_number() {
        assert_eq!(
            mysql_value_to_json(mysql::Value::Int(-42)),
            Value::Number((-42_i64).into())
        );
    }

    #[test]
    fn bytes_maps_to_json_string() {
        assert_eq!(
            mysql_value_to_json(mysql::Value::Bytes(b"hello".to_vec())),
            Value::String("hello".to_string())
        );
    }

    #[test]
    fn binary_bytes_map_to_placeholder() {
        let result = mysql_value_to_json(mysql::Value::Bytes(vec![0xFF, 0xFE]));
        assert_eq!(result, Value::String("<binary data>".to_string()));
    }

    #[test]
    fn date_formats_as_iso_string() {
        let result = mysql_value_to_json(mysql::Value::Date(2024, 3, 15, 10, 30, 0, 0));
        assert_eq!(result, Value::String("2024-03-15 10:30:00.000000".to_string()));
    }
}
