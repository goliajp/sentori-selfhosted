//! Per-table ETL counters.

use std::collections::BTreeMap;

#[derive(Default, Debug)]
pub struct Report {
    /// Source row counts per table (read).
    pub read: BTreeMap<String, u64>,
    /// Destination row counts per table (inserted).
    pub written: BTreeMap<String, u64>,
    /// Rows skipped (conflict on PK, already migrated).
    pub skipped: BTreeMap<String, u64>,
    /// Rows failed (validation or unexpected error).
    pub failed: BTreeMap<String, u64>,
}

impl Report {
    pub fn note_read(&mut self, table: &str, n: u64) {
        *self.read.entry(table.to_string()).or_default() += n;
    }
    pub fn note_written(&mut self, table: &str, n: u64) {
        *self.written.entry(table.to_string()).or_default() += n;
    }
    pub fn note_skipped(&mut self, table: &str, n: u64) {
        *self.skipped.entry(table.to_string()).or_default() += n;
    }
    pub fn note_failed(&mut self, table: &str, n: u64) {
        *self.failed.entry(table.to_string()).or_default() += n;
    }

    /// Print a summary table to stdout.
    pub fn print(&self) {
        println!("\n──── ETL summary ────");
        println!(
            "{:<24} {:>10} {:>10} {:>10} {:>10}",
            "table", "read", "written", "skipped", "failed"
        );
        let all: std::collections::BTreeSet<_> = self
            .read
            .keys()
            .chain(self.written.keys())
            .chain(self.skipped.keys())
            .chain(self.failed.keys())
            .cloned()
            .collect();
        for t in all {
            println!(
                "{:<24} {:>10} {:>10} {:>10} {:>10}",
                t,
                self.read.get(&t).copied().unwrap_or(0),
                self.written.get(&t).copied().unwrap_or(0),
                self.skipped.get(&t).copied().unwrap_or(0),
                self.failed.get(&t).copied().unwrap_or(0),
            );
        }
        println!("──────────────────────\n");
    }
}
