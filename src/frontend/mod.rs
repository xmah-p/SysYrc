mod koopa_context;
mod koopa_generator;
mod symbol_table;
mod array_init_helper;

use std::io;

use koopa::ir::Program;

use koopa::back::KoopaGenerator;
use koopa_context::KoopaContext;
use koopa_generator::GenerateKoopa;


pub fn translate_to_koopa(cu: crate::ast::CompUnit) -> Program {
    koopa::ir::Type::set_ptr_size(4);
    let mut prog = Program::new();
    let mut context = KoopaContext::new(&mut prog);
    cu.generate(&mut context);
    prog
}

pub fn emit_ir(program: &Program, output: impl io::Write) -> Result<(), std::io::Error> {
    KoopaGenerator::new(output).generate_on(program)
}
