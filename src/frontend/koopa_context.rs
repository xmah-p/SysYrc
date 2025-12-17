use koopa::ir::builder::{BasicBlockBuilder, LocalBuilder};
use koopa::ir::entities::{ValueData, ValueKind};
use koopa::ir::*;

use crate::frontend::symbol_table::SymbolTable;

/// Context for Koopa IR generation
pub struct KoopaContext<'a> {
    pub program: &'a mut Program,
    pub symbol_table: SymbolTable,
    current_func: Option<Function>,
    current_bb: Option<BasicBlock>,
    bb_count: usize, // For generating unique basic block names
    // These two stacks are used to keep track of the current loop's
    // break and continue targets
    // For while loops, they should always be operated in pairs
    loop_break_stack: Vec<BasicBlock>,
    loop_continue_stack: Vec<BasicBlock>,
}

impl<'a> KoopaContext<'a> {
    pub fn new(program: &'a mut Program) -> Self {
        KoopaContext {
            program,
            current_func: None,
            current_bb: None,
            symbol_table: SymbolTable::new(),
            bb_count: 0,
            loop_break_stack: Vec::new(),
            loop_continue_stack: Vec::new(),
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

    pub fn set_current_func(&mut self, func: Function) {
        self.current_func = Some(func);
        self.bb_count = 0;
        self.current_bb = None;
        assert!(self.loop_break_stack.is_empty());
        assert!(self.loop_continue_stack.is_empty());
    }

    pub fn get_current_bb(&self) -> BasicBlock {
        self.current_bb
            .expect("Current basic block is not set in KoopaContext")
    }

    pub fn set_current_bb(&mut self, bb: BasicBlock) {
        self.current_bb = Some(bb);
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

    /// Pushes the break and continue target basic blocks of the current loop
    /// onto their respective stacks
    pub fn enter_loop(&mut self, break_target: BasicBlock, continue_target: BasicBlock) {
        self.loop_break_stack.push(break_target);
        self.loop_continue_stack.push(continue_target);
    }

    /// Pops the break and continue target basic blocks of the current loop
    /// from their respective stacks
    pub fn exit_loop(&mut self) {
        self.loop_break_stack.pop();
        self.loop_continue_stack.pop();
    }

    /// Clones and returns the current loop's continue target basic block
    pub fn get_current_loop_break_target(&self) -> BasicBlock {
        self.loop_break_stack
            .last()
            .expect("No current loop break target found")
            .clone()
    }

    /// Clones and returns the current loop's continue target basic block
    pub fn get_current_loop_continue_target(&self) -> BasicBlock {
        self.loop_continue_stack
            .last()
            .expect("No current loop continue target found")
            .clone()
    }

    pub fn is_current_bb_terminated(&mut self) -> bool {
        let current_bb = self.get_current_bb();
        let func_data = self.current_func_mut();

        let bb_node = func_data.layout_mut().bb_mut(current_bb);
        if let Some(&last_inst) = bb_node.insts().back_key() {
            let inst_data = func_data.dfg().value(last_inst);
            match inst_data.kind() {
                ValueKind::Branch(_) | ValueKind::Jump(_) | ValueKind::Return(_) => true,
                _ => false,
            }
        } else {
            false // No instructions in the current basic block
        }
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
    pub fn new_bb(&mut self, name_prefix: &str) -> BasicBlock {
        let name = format!("{}_{}", name_prefix, self.bb_count);
        self.bb_count += 1;
        self.current_func_mut().dfg_mut().new_bb().basic_block(Some(name))
    }
}
