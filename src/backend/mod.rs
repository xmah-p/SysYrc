mod riscv_context;
mod riscv_generator;

use koopa::ir::Program;
use riscv_context::RiscvContext;
use riscv_generator::GenerateRiscv;
use std::io;

pub fn emit_riscv(program: &Program, mut writer: impl io::Write) -> io::Result<()> {
    let mut context = RiscvContext::new(program);
    program.generate(&mut context).unwrap(); // [TODO] Error handling
    write!(writer, "{}", context.get_output())?;
    Ok(())
}
