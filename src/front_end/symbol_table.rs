use std::collections::HashMap;
use koopa::ir::{
    Value,
};

/// Symbol table for Koopa IR generation
/// Outer table is owned by the current table
/// Top-level table has None as outer
/// Only the most inner table is owned by KoopaContext
pub struct SymbolTable {
    table: HashMap<String, Value>,
    outer: Option<Box<SymbolTable>>,
}

impl SymbolTable {
    /// Creates an empty global symbol table
    pub fn new() -> Self {
        SymbolTable {
            table: HashMap::new(),
            outer: None,
        }
    }

    pub fn lookup(&self, name: &str) -> Option<&Value> {
        if let Some(val) = self.table.get(name) {
            Some(val)
        } else if let Some(outer_table) = &self.outer {
            outer_table.lookup(name)
        } else {
            None
        }
    }

    pub fn insert(&mut self, name: String, value: Value) {
        self.table.insert(name, value);
    }

    pub fn remove(&mut self, name: &str) {
        self.table.remove(name);
    }

    pub fn enter_scope(&mut self) {
        let new_table = SymbolTable {
            table: HashMap::new(),
            outer: Some(Box::new(std::mem::replace(self, SymbolTable::new()))),
        };
        *self = new_table;
    }

    pub fn exit_scope(&mut self) {
        if let Some(outer_table) = self.outer.take() {
            *self = *outer_table;
        } else {
            panic!("No outer scope to exit to");
        }
    }
}
