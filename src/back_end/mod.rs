mod context;
mod generator;

use koopa::ir::Program;
use context::RiscvContext;
use generator::GenerateRiscv;
use std::io;

pub fn emit_riscv(program: &Program, mut writer: impl io::Write) -> io::Result<()> {
    let mut context = RiscvContext::new();
    program.generate(&mut context).unwrap(); // [TODO] Error handling
    write!(writer, "{}", context.get_output())?;
    Ok(())
}
