use crate::Evaluator;
use crate::expr::{Expr, expr_to_string};

impl Evaluator {
    /// (sql-open path) or (sql-open path name)
    /// Opens a SQLite database. Use ":memory:" for in-memory.
    /// Returns the connection name (for use with other sql-* functions).
    pub(crate) fn builtin_sql_open(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.is_empty() {
            return Err("(sql-open path [name]) requires at least 1 argument".into());
        }

        let path = expr_to_string(&args[0]);
        let name = if args.len() > 1 {
            expr_to_string(&args[1])
        } else {
            "default".to_string()
        };

        let conn = rusqlite::Connection::open(&path)
            .map_err(|e| format!("sql-open: failed to open '{}': {}", path, e))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| format!("sql-open: pragma failed: {}", e))?;

        self.sql_connections.insert(name.clone(), conn);
        Ok(Expr::Str(name))
    }

    /// (sql-execute sql [params...])
    /// Executes a SQL statement (INSERT, UPDATE, DELETE, CREATE, etc.)
    /// Returns the number of affected rows as a number.
    pub(crate) fn builtin_sql_execute(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.is_empty() {
            return Err("(sql-execute sql [params...]) requires at least 1 argument".into());
        }

        let sql = expr_to_string(&args[0]);
        let conn = self.get_default_conn()?;

        let param_values: Vec<Box<dyn rusqlite::types::ToSql>> = args[1..]
            .iter()
            .map(|a| -> Box<dyn rusqlite::types::ToSql> {
                match a {
                    Expr::Num(n) => Box::new(*n),
                    Expr::Str(s) => Box::new(s.clone()),
                    Expr::Bool(b) => Box::new(*b),
                    Expr::Nil => Box::new(rusqlite::types::Value::Null),
                    _ => Box::new(expr_to_string(a)),
                }
            })
            .collect();

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let affected = conn
            .execute(&sql, params_refs.as_slice())
            .map_err(|e| format!("sql-execute: {}", e))?;

        Ok(Expr::Num(affected as f64))
    }

    /// (sql-query sql [params...])
    /// Executes a SELECT query and returns results as a list of lists.
    /// Each row is a list of values. First call also returns column names.
    /// Returns: ((columns...) (row1...) (row2...) ...)
    pub(crate) fn builtin_sql_query(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.is_empty() {
            return Err("(sql-query sql [params...]) requires at least 1 argument".into());
        }

        let sql = expr_to_string(&args[0]);
        let conn = self.get_default_conn()?;

        let param_values: Vec<Box<dyn rusqlite::types::ToSql>> = args[1..]
            .iter()
            .map(|a| -> Box<dyn rusqlite::types::ToSql> {
                match a {
                    Expr::Num(n) => Box::new(*n),
                    Expr::Str(s) => Box::new(s.clone()),
                    Expr::Bool(b) => Box::new(*b),
                    Expr::Nil => Box::new(rusqlite::types::Value::Null),
                    _ => Box::new(expr_to_string(a)),
                }
            })
            .collect();

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("sql-query: prepare failed: {}", e))?;

        let column_names: Vec<Expr> = stmt
            .column_names()
            .iter()
            .map(|name| Expr::Str(name.to_string()))
            .collect();

        let num_columns = stmt.column_count();

        let mut rows_result = stmt
            .query(params_refs.as_slice())
            .map_err(|e| format!("sql-query: query failed: {}", e))?;

        let mut rows: Vec<Expr> = Vec::new();
        rows.push(Expr::List(column_names));

        while let Some(row) = rows_result
            .next()
            .map_err(|e| format!("sql-query: row error: {}", e))?
        {
            let mut values = Vec::new();
            for i in 0..num_columns {
                let val = match row.get::<_, Option<String>>(i) {
                    Ok(Some(s)) => Expr::Str(s),
                    Ok(None) => {
                        // Try other types
                        match row.get::<_, Option<f64>>(i) {
                            Ok(Some(n)) => Expr::Num(n),
                            Ok(None) => match row.get::<_, Option<bool>>(i) {
                                Ok(Some(b)) => Expr::Bool(b),
                                _ => Expr::Nil,
                            },
                            Err(_) => Expr::Nil,
                        }
                    }
                    Err(_) => {
                        // Try as number
                        match row.get::<_, Option<f64>>(i) {
                            Ok(Some(n)) => Expr::Num(n),
                            Ok(None) => Expr::Nil,
                            Err(_) => Expr::Nil,
                        }
                    }
                };
                values.push(val);
            }
            rows.push(Expr::List(values));
        }

        Ok(Expr::List(rows))
    }

    /// (sql-tables) - Lists all tables in the default database
    pub(crate) fn builtin_sql_tables(&mut self, _args: &[Expr]) -> Result<Expr, String> {
        let conn = self.get_default_conn()?;
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .map_err(|e| format!("sql-tables: {}", e))?;

        let tables: Vec<Expr> = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| format!("sql-tables: {}", e))?
            .filter_map(|r| r.ok())
            .map(|name| Expr::Str(name))
            .collect();

        Ok(Expr::List(tables))
    }

    /// (sql-schema table) - Returns the CREATE TABLE statement for a table
    pub(crate) fn builtin_sql_schema(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.is_empty() {
            return Err("(sql-schema table) requires a table name".into());
        }

        let table = expr_to_string(&args[0]);
        let conn = self.get_default_conn()?;

        let sql = format!(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='{}'",
            table.replace('\'', "''")
        );

        let result: Option<String> = conn
            .query_row(&sql, [], |row| row.get(0))
            .map_err(|e| format!("sql-schema: {}", e))?;

        match result {
            Some(create_sql) => Ok(Expr::Str(create_sql)),
            None => Err(format!("sql-schema: table '{}' not found", table)),
        }
    }

    /// (sql-close [name]) - Closes a database connection
    pub(crate) fn builtin_sql_close(&mut self, args: &[Expr]) -> Result<Expr, String> {
        let name = if args.is_empty() {
            "default".to_string()
        } else {
            expr_to_string(&args[0])
        };

        match self.sql_connections.remove(&name) {
            Some(_) => Ok(Expr::Bool(true)),
            None => Err(format!("sql-close: connection '{}' not found", name)),
        }
    }

    fn get_default_conn(&self) -> Result<&rusqlite::Connection, String> {
        self.sql_connections
            .get("default")
            .ok_or_else(|| "no default database opened. Use (sql-open path) first".to_string())
    }
}
