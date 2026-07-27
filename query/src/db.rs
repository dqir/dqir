use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryLogEntry {
    pub query: String,
    pub fn_name: String,
}

#[salsa::db]
pub trait Db: salsa::Database {
    fn log_query(&self, query: &str, fn_name: &str);
    fn get_logs(&self) -> Vec<QueryLogEntry>;
    fn clear_logs(&self);
}

#[derive(Default)]
#[salsa::db]
pub struct CompilerDatabase {
    storage: salsa::Storage<Self>,
    pub logs: Arc<Mutex<Vec<QueryLogEntry>>>,
}

#[salsa::db]
impl salsa::Database for CompilerDatabase {}

#[salsa::db]
impl Db for CompilerDatabase {
    fn log_query(&self, query: &str, fn_name: &str) {
        if let Ok(mut lock) = self.logs.lock() {
            lock.push(QueryLogEntry {
                query: query.to_string(),
                fn_name: fn_name.to_string(),
            });
        }
    }

    fn get_logs(&self) -> Vec<QueryLogEntry> {
        if let Ok(lock) = self.logs.lock() {
            lock.clone()
        } else {
            Vec::new()
        }
    }

    fn clear_logs(&self) {
        if let Ok(mut lock) = self.logs.lock() {
            lock.clear();
        }
    }
}
