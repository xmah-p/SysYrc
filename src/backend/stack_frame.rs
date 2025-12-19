use koopa::ir::entities::{FunctionData, Value, ValueKind};
use std::cmp::max;
use std::collections::HashMap;

use crate::backend::riscv_generator::WORD_SIZE;

pub struct StackFrame {
    values_map: HashMap<Value, i32>, // Map Koopa IR Values to their stack offsets
    stack_size: i32,                 // Total size of the stack frame
    ra_offset: Option<i32>,          // Offset for the return address if saved
}

impl StackFrame {
    pub fn new() -> Self {
        Self {
            values_map: HashMap::new(),
            stack_size: 0,
            ra_offset: None,
        }
    }

    /// Initializes the stack frame by calculating offsets for each Value
    /// and setting the total stack size.
    /// Stack frame layout:
    ///
    /// Stack frame for previous function
    /// Saved ra
    /// Local variables...
    /// 10th argument
    /// 9th argument
    /// Stack frame for Next function
    pub fn initialize(&mut self, func: &FunctionData) {
        self.values_map.clear();

        let mut has_call = false;
        let mut max_call_args = 0;
        for (&_, node) in func.layout().bbs() {
            for &inst in node.insts().keys() {
                let inst_data = func.dfg().value(inst);
                if let ValueKind::Call(call) = inst_data.kind() {
                    has_call = true;
                    max_call_args = max(max_call_args, call.args().len());
                }
            }
        }
        let ra_size = if has_call { WORD_SIZE } else { 0 };
        let call_args_size = if max_call_args > 8 {
            (max_call_args - 8) as i32 * WORD_SIZE
        } else {
            0
        };

        let mut local_size = 0;
        for (&_, node) in func.layout().bbs() {
            for &inst in node.insts().keys() {
                let inst_data = func.dfg().value(inst);
                if !inst_data.ty().is_unit() {
                    self.values_map.insert(inst, local_size + call_args_size);
                    local_size += WORD_SIZE;
                }
            }
        }

        let total_size = ra_size + local_size + call_args_size;
        self.stack_size = (total_size + 15) & !15; // Align to 16 bytes
        self.ra_offset = if has_call {
            Some(self.stack_size - ra_size)
        } else {
            None
        };
    }

    pub fn get_stack_offset(&self, value: Value) -> i32 {
        self.values_map
            .get(&value)
            .copied()
            .expect("Value not found in stack frame")
    }

    pub fn get_stack_size(&self) -> i32 {
        self.stack_size
    }

    pub fn get_ra_offset(&self) -> Option<i32> {
        self.ra_offset
    }
}
