mod context;
mod generator;
mod symbol_table;

use std::io;

use koopa::ir::Program;

use koopa::back::KoopaGenerator;
use context::KoopaContext;
use generator::GenerateKoopa;


pub fn translate_to_koopa(cu: crate::ast::CompUnit) -> Program {
    let mut prog = Program::new();
    let mut context = KoopaContext::new(&mut prog);
    cu.generate(&mut context);
    prog
}

pub fn emit_ir(program: &Program, output: impl io::Write) -> Result<(), std::io::Error> {
    KoopaGenerator::new(output).generate_on(program)
}
