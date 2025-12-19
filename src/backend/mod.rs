mod riscv_generator;
mod asm_writer;
mod stack_frame;

use koopa::ir::Program;
use riscv_generator::RiscvGenerator;
use std::io;

pub fn emit_riscv(program: &Program, mut writer: impl io::Write) -> io::Result<()> {
    let mut generator = RiscvGenerator::new(program, writer);
    generator.generate_program()
}
