use std::collections::HashMap;
use std::fmt;
use std::fmt::Write; // to bring the write! macro into scope

use koopa::ir::entities::ValueData;
use koopa::ir::{Function, FunctionData, Program, Value, ValueKind};

/// Context for RISC-V code generation
pub struct RiscvContext<'a> {
    // Accumulates the generated RISC-V code
    out: String,
    pub program: Option<&'a Program>,
    pub current_func: Option<Function>,    // Current function being processed
    pub current_value: Option<Value>,      // Current value being processed

    values_map: HashMap<Value, i32>, // Map Koopa IR Values to their stack offsets
    stack_size: i32,                 // Total size of the stack frame
}

impl<'a> Default for RiscvContext<'a> {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn get_value_data(&self, value: Value) -> &ValueData {
        let func = Self::func_data(self.program, self.current_func);
        func.dfg().value(value)
    }

    fn func_data(program: Option<&'a Program>, func: Option<Function>) -> &'a FunctionData {
        program
            .expect("Program is not set in RiscvContext")
            .func(func.expect("Current function is not set in RiscvContext"))
    }

    /// Initializes the stack frame by calculating offsets for each Value
    /// and setting the total stack size.
    pub fn init_stack_frame(&mut self) {
        self.values_map.clear();
        self.stack_size = 0;

        let func = Self::func_data(self.program, self.current_func);

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
        self.stack_size = (self.stack_size + 15) & !15;
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

    pub fn load_value_to_reg(&mut self, value: Value, reg_name: &str) -> fmt::Result {
        let value_data = self.get_value_data(value);

        match value_data.kind() {
            ValueKind::Integer(int) => {
                if int.value() == 0 {
                    self.write_inst(&format!("mv {}, x0", reg_name))
                } else {
                    self.write_inst(&format!("li {}, {}", reg_name, int.value()))
                }
            }
            // Result of other instructions
            // They should be already stored in the stack
            _ => {
                let offset = self.get_stack_offset(value);
                if offset > 2047 {
                    self.write_inst(&format!("li t0, {}", offset))?;
                    self.write_inst(&format!("add t0, sp, t0"))?;
                    return self.write_inst(&format!("lw {}, 0(t0)", reg_name));
                }
                self.write_inst(&format!("lw {}, {}(sp)", reg_name, offset))
            }
        }
    }

    pub fn save_value_to_reg(&mut self, value: Value, reg_name: &str) -> fmt::Result {
        if self.get_value_data(value).ty().is_unit() {
            return Ok(());
        }

        let offset = self.get_stack_offset(value);
        if offset > 2047 {
            self.write_inst(&format!("li t0, {}", offset))?;
            self.write_inst(&format!("add t0, sp, t0"))?;
            return self.write_inst(&format!("sw {}, 0(t0)", reg_name));
        }
        self.write_inst(&format!("sw {}, {}(sp)", reg_name, offset))
    }
}
