use std::collections::HashMap;
use std::fmt;
use std::fmt::Write; // to bring the write! macro into scope

use koopa::ir::entities::{ValueData, ValueKind};
use koopa::ir::{FunctionData, Program, Value};

/// Context for RISC-V code generation
pub struct RiscvContext<'a> {
    // Accumulates the generated RISC-V code
    out: String,
    // Reference to the Koopa IR program
    pub program: Option<&'a Program>,
    // Current function being processed
    // This is to access ValueData from Value during generation
    pub current_func: Option<&'a FunctionData>,
    // [TODO] Maybe change FunctionData to Function?
    pub current_value: Option<Value>,

    values_map: HashMap<Value, i32>, // Map Koopa IR Values to their stack offsets
    stack_size: i32,                 // Total size of the stack frame
}

impl<'a> RiscvContext<'a> {
    // Rust doesn't generate a default constructor automatically like C++
    // new() is just a regular static method conventionally used as a constructor
    pub fn new() -> Self {
        RiscvContext {
            out: String::new(),
            program: None,
            current_func: None,
            current_value: None,
            values_map: HashMap::new(),
            stack_size: 0,
        }
    }

    pub fn write_line(&mut self, content: &str) -> fmt::Result {
        writeln!(self.out, "{}", content)
    }

    /// Writes an instruction line with indentation.
    pub fn write_inst(&mut self, content: &str) -> fmt::Result {
        writeln!(self.out, "    {}", content)
    }

    pub fn get_output(&self) -> &str {
        &self.out
    }

    pub fn get_value_from_func(&self, value: Value) -> &ValueData {
        let func = self
            .current_func
            .expect("Current function is not set in RiscvContext");
        func.dfg().value(value)
    }

    /// Initializes the stack frame by calculating offsets for each Value
    /// and setting the total stack size.
    pub fn init_stack_frame(&mut self) {
        self.values_map.clear();
        self.stack_size = 0;

        let func = self
            .current_func
            .expect("Current function is not set in RiscvContext");

        for (&_bb, node) in func.layout().bbs() {
            for &inst in node.insts().keys() {
                let inst_data: &ValueData = func.dfg().value(inst);
                // [TODO]: Assume `alloc` instructions always allocate 4 bytes for now
                if !inst_data.ty().is_unit() {
                    self.values_map.insert(inst, self.stack_size);
                    self.stack_size += 4; // Assuming each non-unit value takes 4 bytes
                }
            }
        }
        // Align stack size to 16 bytes
        let alignment = 16;
        self.stack_size = (self.stack_size + alignment - 1) / alignment * alignment;
    }

    pub fn get_stack_offset(&self, value: Value) -> i32 {
        self.values_map.get(&value).cloned().expect("Value not found in stack frame")
    }

    pub fn get_stack_size(&self) -> i32 {
        self.stack_size
    }

    pub fn load_value_to_reg(&mut self, value: Value, reg_name: &str) -> fmt::Result {
        let value_data = self.get_value_from_func(value);

        match value_data.kind() {
            ValueKind::Integer(int) => {
                if int.value() == 0 {
                    self.write_inst(&format!("mv {}, x0", reg_name)) // 优化：移动零寄存器
                } else {
                    self.write_inst(&format!("li {}, {}", reg_name, int.value()))
                }
            }
            // Result of other instructions
            // They should be already stored in the stack
            _ => {
                let offset = self.get_stack_offset(value);
                // [TODO]: Handle the case where offset > 2047
                self.write_inst(&format!("lw {}, {}(sp)", reg_name, offset))
            }
        }
    }

    pub fn save_value_to_reg(&mut self, value: Value, reg_name: &str) -> fmt::Result {
        if self.get_value_from_func(value).ty().is_unit() {
            return Ok(());
        }

        let offset = self.get_stack_offset(value);
        // [TODO]: Handle the case where offset > 2047
        self.write_inst(&format!("sw {}, {}(sp)", reg_name, offset))
    }
}
