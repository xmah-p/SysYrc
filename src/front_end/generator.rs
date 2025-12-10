use core::panic;

use super::context::KoopaContext;

use crate::ast::*;
use koopa::ir::values::BinaryOp as KoopaBinaryOp;
use koopa::ir::{builder_traits::*, BasicBlock, FunctionData, Type, Value};

/// Trait for generating Koopa IR entities
pub trait GenerateKoopa {
    fn generate(&self, context: &mut KoopaContext) -> ();
}

impl GenerateKoopa for CompUnit {
    fn generate(&self, context: &mut KoopaContext) -> () {
        // Currently only supports one function definition
        let func_def = &self.func_def;
        func_def.generate(context);
    }
}

impl GenerateKoopa for FuncDef {
    fn generate(&self, context: &mut KoopaContext) -> () {
        let func_type = match self.func_type {
            FuncType::Int => Type::get_i32(),
        };

        let func_data =
            FunctionData::new(std::format!("@{}", self.func_name), Vec::new(), func_type);
        let func = context.program.new_func(func_data);
        context.set_current_func(func);

        self.block.generate(context);
    }
}

impl GenerateKoopa for Block {
    fn generate(&self, context: &mut KoopaContext) -> () {
        let entry_bb: BasicBlock = context.new_bb().basic_block(Some("%entry".into()));
        context.add_bb(entry_bb);
        context.set_current_bb(entry_bb);

        for item in &self.items {
            match item {
                BlockItem::Stmt(stmt) => stmt.generate(context),
                BlockItem::Decl(decl) => decl.generate(context),
            }
        }
    }
}

impl GenerateKoopa for Decl {
    fn generate(&self, context: &mut KoopaContext) -> () {
        ()
    }
}

impl GenerateKoopa for Stmt {
    fn generate(&self, context: &mut KoopaContext) -> () {
        match self {
            Stmt::Return { expr } => {
                let value: Value = expr.generate(context);
                let inst: Value = context.new_value().ret(Some(value));
                context.add_inst(inst);

            },
            _ => panic!("Unsupported statement"),
        }
    }
}

impl Expr {
    fn generate(&self, context: &mut KoopaContext) -> Value {
        match self {
            Expr::Number(n) => {
                let value = context.new_value().integer(*n);
                value
            }
            Expr::Binary { op, lhs, rhs } => {
                let lhs_value = lhs.generate(context);
                let rhs_value = rhs.generate(context);

                if let Some(koopa_op) = map_binary_op(*op) {
                    let inst = context.new_value().binary(koopa_op, lhs_value, rhs_value);
                    context.add_inst(inst);

                    inst
                } else {
                    // Handles logical and/or
                    let zero = context.new_value().integer(0);

                    let lhs_bool =
                        context
                            .new_value()
                            .binary(KoopaBinaryOp::NotEq, lhs_value, zero);
                    context.add_inst(lhs_bool);

                    let rhs_bool =
                        context
                            .new_value()
                            .binary(KoopaBinaryOp::NotEq, rhs_value, zero);
                    context.add_inst(rhs_bool);

                    let logic_op = match op {
                        BinaryOp::And => KoopaBinaryOp::And,
                        BinaryOp::Or => KoopaBinaryOp::Or,
                        _ => unreachable!("Already handled by map_binary_op"),
                    };

                    let inst = context.new_value().binary(logic_op, lhs_bool, rhs_bool);
                    context.add_inst(inst);
                    inst
                }
            }
            Expr::Unary { op, expr } => match op {
                UnaryOp::Pos => expr.generate(context),
                UnaryOp::Neg => {
                    let value = expr.generate(context);
                    let zero = context.new_value().integer(0);
                    let inst = context.new_value().binary(KoopaBinaryOp::Sub, zero, value);
                    context.add_inst(inst);
                    inst
                }
                UnaryOp::Not => {
                    let value = expr.generate(context);
                    let zero = context.new_value().integer(0);
                    let inst = context.new_value().binary(KoopaBinaryOp::Eq, value, zero);
                    context.add_inst(inst);
                    inst
                }
            },
            Expr::LVal(name) => {
                panic!("LValue not support yet");
            }
        }
    }
}

fn map_binary_op(op: BinaryOp) -> Option<KoopaBinaryOp> {
    match op {
        BinaryOp::Add => Some(KoopaBinaryOp::Add),
        BinaryOp::Sub => Some(KoopaBinaryOp::Sub),
        BinaryOp::Mul => Some(KoopaBinaryOp::Mul),
        BinaryOp::Div => Some(KoopaBinaryOp::Div),
        BinaryOp::Mod => Some(KoopaBinaryOp::Mod),
        BinaryOp::Eq => Some(KoopaBinaryOp::Eq),
        BinaryOp::Neq => Some(KoopaBinaryOp::NotEq),
        BinaryOp::Lt => Some(KoopaBinaryOp::Lt),
        BinaryOp::Gt => Some(KoopaBinaryOp::Gt),
        BinaryOp::Leq => Some(KoopaBinaryOp::Le),
        BinaryOp::Geq => Some(KoopaBinaryOp::Ge),
        // And/Or are handled separately in the main logic
        BinaryOp::And | BinaryOp::Or => None,
    }
}
