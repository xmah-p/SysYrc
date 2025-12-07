use std::fmt;
use std::fmt::Write; // to bring the write! macro into scope

use koopa::ir::entities::ValueData;
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
}

impl<'a> RiscvContext<'a> {
    // Rust doesn't generate a default constructor automatically like C++
    // new() is just a regular static method conventionally used as a constructor
    pub fn new() -> Self {
        RiscvContext {
            out: String::new(),
            program: None,
            current_func: None,
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
}
