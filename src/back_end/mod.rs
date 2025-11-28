
use koopa::{
    front::ast::Error,
    ir::{FunctionData, Program, Type},
};
use std::io::Write;

fn compile_riscv(koopa_ir: Program) -> Result<String, Error> {
    panic!("RISC-V backend not implemented yet");
}

fn emit_riscv(program: &Program, output: impl io::Write) -> Result<(), std::io::Error> {
    let riscv_code = compile_riscv(program.clone())?;
    let mut writer = output;
    writer.write_all(riscv_code.as_bytes())?;
    Ok(())
}
