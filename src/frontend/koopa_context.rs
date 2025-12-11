use koopa::ir::*;
use koopa::ir::entities::{ValueData, ValueKind};
use koopa::ir::builder::{BlockBuilder, LocalBuilder};

use crate::frontend::symbol_table::SymbolTable;

/// Context for Koopa IR generation
pub struct KoopaContext<'a> {
    pub program: &'a mut Program,
    pub symbol_table: SymbolTable,
    current_func: Option<Function>,
    current_bb: Option<BasicBlock>,
}

impl<'a> KoopaContext<'a> {
    pub fn new(program: &'a mut Program) -> Self {
        KoopaContext {
            program,
            current_func: None,
            current_bb: None,
            symbol_table: SymbolTable::new(),
        }
    }

    pub fn current_func_mut(&mut self) -> &mut FunctionData {
        self.program.func_mut(
            self.current_func
                .expect("Current function is not set in KoopaContext"),
        )
    }

    pub fn current_func(&self) -> &FunctionData {
        self.program.func(
            self.current_func
                .expect("Current function is not set in KoopaContext"),
        )
    }

    pub fn get_value_data(&self, value: Value) -> &ValueData {
        self.current_func().dfg().value(value)
    }

    pub fn get_value_kind(&self, value: Value) -> ValueKind {
        self.get_value_data(value).kind().clone()
    }

    pub fn set_value_name(&mut self, value: Value, name: String) {
        self.current_func_mut()
            .dfg_mut()
            .set_value_name(value, Some(name));
    }

    pub fn set_current_func(&mut self, func: Function) {
        self.current_func = Some(func);
    }

    pub fn set_current_bb(&mut self, bb: BasicBlock) {
        self.current_bb = Some(bb);
    }

    /// Pushes basic block `bb` to the end of the basic block list of
    /// the current function
    pub fn add_bb(&mut self, bb: BasicBlock) {
        self.current_func_mut()
            .layout_mut()
            .bbs_mut()
            .push_key_back(bb)
            .expect("Failed to add basic block");
    }

    /// Pushes instruction `inst` to the end of the instruction list
    /// of the current basic block in the current function
    pub fn add_inst(&mut self, inst: Value) {
        let bb = self
            .current_bb
            .expect("Current basic block is not set in KoopaContext");
        self.current_func_mut()
            .layout_mut()
            .bb_mut(bb)
            .insts_mut()
            .push_key_back(inst)
            .expect("Failed to add instruction");
    }

    /// Creates a new value in the DataFlow Graph of the current function
    /// Returns a LocalBuilder for the newly created value
    pub fn new_value(&mut self) -> LocalBuilder {
        self.current_func_mut().dfg_mut().new_value()
    }

    /// Creates a new basic block in the DFG of func
    /// Returns a BasicBlockBuilder for the newly created basic block
    pub fn new_bb(&mut self) -> BlockBuilder {
        self.current_func_mut().dfg_mut().new_bb()
    }
}
