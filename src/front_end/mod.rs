use std::io;

use koopa::ir::builder_traits::*;
use koopa::{
    front::ast::Error,
    ir::{FunctionData, Program, Type},
};

use koopa::back::KoopaGenerator;

use crate::ast::{FuncDef, FuncType};

// 我们应该生成一个 Koopa IR 程序.
// 程序中有一个名字叫 main 的函数.
// 函数里有一个入口基本块.
// 基本块里有一条返回指令.
// 返回指令的返回值就是 SysY 里 return 语句后跟的值, 也就是一个整数常量.

macro_rules! new_bb {
    ($func:expr) => {
        $func.dfg_mut().new_bb()
    };
}

macro_rules! new_value {
    ($func:expr) => {
        $func.dfg_mut().new_value()
    };
}

macro_rules! add_bb {
    ($func:expr, $bb:expr) => {
        $func.layout_mut().bbs_mut().push_key_back($bb).unwrap()
    };
}

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
    let name = func_def.ident;
    let block = func_def.block;
    let stmt = block.stmt;
    let num = stmt.num;

    let func_data_type = match func_type {
        FuncType::Int => Type::get_i32(),
    };
    let func_data = FunctionData::new(std::format!("@{}", name), Vec::new(), func_data_type);

    let func = prog.new_func(func_data);

    let func = prog.func_mut(func);
    let entry = new_bb!(func).basic_block(Some("%entry".into()));
    add_bb!(func, entry);

    let num = new_value!(func).integer(num);

    let ret = new_value!(func).ret(Some(num));
    add_inst!(func, entry, ret);
    Ok(prog)
}

pub fn emit_lr(program: &Program, output: impl io::Write) -> Result<(), std::io::Error> {
    KoopaGenerator::new(output).generate_on(program)
}
