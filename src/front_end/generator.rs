use super::context::KoopaContext;

use koopa::ir::{
    builder_traits::*,
    FunctionData, Value, Type, BasicBlock
};

use crate::ast::*;

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
            FunctionData::new(std::format!("@{}", self.identifier), Vec::new(), func_type);
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
        self.stmt.generate(context);
    }
}

impl GenerateKoopa for Stmt {
    fn generate(&self, context: &mut KoopaContext) -> () {
        let expr = &self.expr;
        let value: i32 = compute_expr(expr);
        let value: Value = context.new_value().integer(value);
        let inst: Value = context.new_value().ret(Some(value));
        context.add_inst(inst);
    }
}

fn compute_expr(expr: &Expr) -> i32 {
    match expr {
        Expr::Number(n) => *n,
        Expr::Binary { op, lhs, rhs } => match op {
            BinaryOp::Add => compute_expr(lhs) + compute_expr(rhs),
            BinaryOp::Div => compute_expr(lhs) / compute_expr(rhs),
            BinaryOp::Eq => (compute_expr(lhs) == compute_expr(rhs)) as i32,
            BinaryOp::Geq => (compute_expr(lhs) >= compute_expr(rhs)) as i32,
            BinaryOp::Gt => (compute_expr(lhs) > compute_expr(rhs)) as i32,
            BinaryOp::Leq => (compute_expr(lhs) <= compute_expr(rhs)) as i32,
            BinaryOp::And => (compute_expr(lhs) != 0 && compute_expr(rhs) != 0) as i32,
            BinaryOp::Lt => (compute_expr(lhs) < compute_expr(rhs)) as i32,
            BinaryOp::Mod => compute_expr(lhs) % compute_expr(rhs),
            BinaryOp::Mul => compute_expr(lhs) * compute_expr(rhs),
            BinaryOp::Neq => (compute_expr(lhs) != compute_expr(rhs)) as i32,
            BinaryOp::Or => (compute_expr(lhs) != 0 || compute_expr(rhs) != 0) as i32,
            BinaryOp::Sub => compute_expr(lhs) - compute_expr(rhs),
        },
        Expr::Unary { op, expr } => match op {
            UnaryOp::Neg => -compute_expr(expr),
            UnaryOp::Not => (compute_expr(expr) == 0) as i32,
            UnaryOp::Pos => compute_expr(expr),
        }
    }
}
