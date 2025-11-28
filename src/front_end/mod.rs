use std::io;

use koopa::ir::builder_traits::*;
use koopa::{
    front::ast::Error,
    ir::{FunctionData, Program, Type},
};

use koopa::back::KoopaGenerator;

use crate::ast::{FuncDef, FuncType};

// Koopa IR 中, 最大的单位是 Program, 它由若干全局变量 Value 和 函数 Function 构成.
// Function 由基本块 Basic Block 构成.
// 基本块中是一系列指令, 指令也是 Value.
// - Program
//     - Value 1
//     - Value 2
//     - ...
//     - Function 1
//         - Basic Block 1
//             - Value 1
//             - Value 2
//             - ...
//         - Basic Block 2
//         - ...
//     - Function 2
//     - ...

// 基本块是一系列指令的集合, 它只有一个入口点且只有一个出口点. 
// 即, 跳转的目标只能是基本块的开头, 且只有最后一条指令能进行控制流的转移

// Value 的种类有：Integer, ZeroInit, Undef, Aggregate, FuncArgRef, 
// BlockArgRef, Alloc, GlobalAlloc, Load, Store, GetPtr, GetElemPtr, 
// Binary, Branch, Jump, Call, Return

// Function, Basic Block, Value 的名字必须以 @ 或者 % 开头. 
// 前者表示这是一个 "具名符号", 后者表示这是一个 "临时符号".

// FunctionData includes a DFG and a Layout
// DFG (DataFlowGraph) holds all data of values (ValueData) and basic
// blocks (BasicBlockData), and maintains their use-define and
// define-use chain.
// Layout maintains the order of instructions (Value) and basic blocks 
// in a function.

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
