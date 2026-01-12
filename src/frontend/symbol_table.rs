use koopa::ir::{Function, Value};
use std::collections::HashMap;

/// Information about a variable in the symbol table
/// For constant variables, their values can be calculated at compile time.
/// We store their Integer values directly.
/// For non-constant variables, we store pointers to their allocated memory,
/// i.e., the Value returned by the `alloc` instruction.
/// For functions, we store the corresponding handles (`Function`)
/// Note that a function cannot have the same name as a global variable in SysY
#[derive(Debug, Copy, Clone)]
pub enum SymbolInfo {
    ConstVariable(Value),
    Variable(Value),
    Function(Function),
}

/// Symbol table for Koopa IR generation
/// Outer table is owned by the current table
/// Top-level table has None as outer
/// Only the most inner table is owned by KoopaContext
pub struct SymbolTable {
    level: i32,                         // Scope level for variable shadowing
    table: HashMap<String, SymbolInfo>, // Symbol names DO NOT start with `@` or `%`!
    outer: Option<Box<SymbolTable>>,
}

impl SymbolTable {
    /// Creates an empty global symbol table
    pub fn new() -> Self {
        SymbolTable {
            table: HashMap::new(),
            level: 0,
            outer: None,
        }
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    pub fn lookup(&self, name: &str) -> SymbolInfo {
        self.lookup_recursive(name).expect(&format!("Variable {} not found", name))
    }

    fn lookup_recursive(&self, name: &str) -> Option<SymbolInfo> {
        if let Some(&val) = self.table.get(name) {
            Some(val)
        } else if let Some(outer_table) = &self.outer {
            outer_table.lookup_recursive(name)
        } else {
            None
        }
    }

    pub fn insert(&mut self, name: String, info: SymbolInfo) {
        self.table.insert(name, info);
    }

    pub fn is_global_scope(&self) -> bool {
        self.level == 0
    }

    pub fn enter_scope(&mut self) {
        let new_table = SymbolTable {
            table: HashMap::new(),
            level: self.level + 1,
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
