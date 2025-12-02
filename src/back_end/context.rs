use std::fmt;
use std::fmt::Write;    // to bring the write! macro into scope

use koopa::ir::FunctionData;

pub struct RiscvContext<'a> {
    out: String,
    pub current_func: Option<&'a FunctionData>,
    // Lifetime parameter 'a indicates that current_func
    // is a reference valid as long as RiscvContext is valid
    // i.e., RiscvContext cannot outlive the FunctionData it references
}

impl<'a> RiscvContext<'a> {
    // Rust doesn't generate a default constructor automatically like C++
    // new() is just a regular static method conventionally used as a constructor
    pub fn new() -> Self {
        RiscvContext {
            out: String::new(),
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
}
