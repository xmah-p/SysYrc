use std::io;

use koopa::ir::builder_traits::*;
use koopa::{
    front::ast::Error,
    ir::{FunctionData, Program, Type},
};

use koopa::back::KoopaGenerator;

use crate::ast::{Expr, FuncType};

// Creates a new basic block in the DFG of func
// Returns a BlockBuilder for the newly created basic block
macro_rules! new_bb {
    ($func:expr) => {
        $func.dfg_mut().new_bb()
    };
}

// Create a new value in the DFG of func
// Returns a ValueBuilder for the newly created value
macro_rules! new_value {
    ($func:expr) => {
        $func.dfg_mut().new_value()
    };
}

// Pushes basic block bb to the end of the basic block list (bbs) of func
macro_rules! add_bb {
    ($func:expr, $bb:expr) => {
        $func.layout_mut().bbs_mut().push_key_back($bb).unwrap()
    };
}

// Pushes instruction inst to the end of the instruction list (insts) 
// of basic block bb
macro_rules! add_inst {
    ($func:expr, $bb:expr, $inst:expr) => {
        $func
            .layout_mut()
            .bb_mut($bb)
            .insts_mut()
            .push_key_back($inst)
            .unwrap()
    };
}

pub fn translate_to_koopa(cu: crate::ast::CompUnit) -> Result<Program, Error> {
    let mut prog = Program::new();
    let func_def = cu.func_def;

    let func_type = func_def.func_type;
    let name = func_def.identifier;
    let block = func_def.block;

    let stmt = block.stmt;
    let expr = stmt.expr;
    let num = match expr {
        Expr::Number(n) => n,
        _ => panic!("Only number expressions are supported in this simplified example"),
    };

    let func_data_type = match func_type {
        FuncType::Int => Type::get_i32(),
    };
    let func_data = FunctionData::new(std::format!("@{}", name), Vec::new(), func_data_type);

    let func = prog.new_func(func_data);

    let func = prog.func_mut(func);
    let entry_bb = new_bb!(func).basic_block(Some("%entry".into()));
    add_bb!(func, entry_bb);

    let num = new_value!(func).integer(num);

    let ret = new_value!(func).ret(Some(num));
    add_inst!(func, entry_bb, ret);
    Ok(prog)
}

pub fn emit_ir(program: &Program, output: impl io::Write) -> Result<(), std::io::Error> {
    KoopaGenerator::new(output).generate_on(program)
}
