use core::panic;

use crate::ast::{*, BinaryOp as AstBinaryOp};
use crate::frontend::{symbol_table::VariableInfo, koopa_context::KoopaContext};
use koopa::ir::{*, builder_traits::*, values::BinaryOp as KoopaBinaryOp};

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

        context.symbol_table.enter_scope();
        self.block.generate(context);
        context.symbol_table.exit_scope();
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
        let name = format!("@{}", self.var_name);
        let var_type = match self.var_type {
            ValueType::Int => Type::get_i32(),
        };
        let is_const = self.constant;
        let init_value: Value;

        // Store variable info in symbol table
        // Constant variable
        // Compute its value at compile time and store the result in symbol table
        if is_const {
            let result: i32 = self
                .init_expr
                .as_ref()
                .expect("Constant declaration must have an initializer")
                .compute_constexpr(context);
            init_value = context.new_value().integer(result);
        }
        // Non-constant variable
        // Allocate space for the variable and store the initial value if exists
        // Save its address in symbol table
        else {
            init_value = context.new_value().alloc(var_type);
            context.set_value_name(init_value, name.clone());

            context.add_inst(init_value);
            if let Some(expr) = &self.init_expr {
                let expr_value = expr.generate(context);
                let store_inst = context.new_value().store(expr_value, init_value);
                context.add_inst(store_inst);
            }
        }
        context.symbol_table.insert(name, init_value, is_const);
    }
}

impl GenerateKoopa for Stmt {
    fn generate(&self, context: &mut KoopaContext) -> () {
        match self {
            Stmt::Return { expr } => {
                let value: Value = expr.generate(context);
                let inst: Value = context.new_value().ret(Some(value));
                context.add_inst(inst);
            }
            Stmt::Assign { lval, expr } => {
                let var_name = format!("@{}", lval);
                let addr: VariableInfo = context
                    .symbol_table
                    .lookup(&var_name)
                    .expect("Variable not found in symbol table");
                match addr {
                    VariableInfo::ConstVariable(_) => {
                        panic!("Cannot assign to a constant variable");
                    }
                    VariableInfo::Variable(var_addr) => {
                        let expr_value = expr.generate(context);
                        let store_inst = context.new_value().store(expr_value, var_addr);
                        context.add_inst(store_inst);
                    }
                }
            }
        }
    }
}

impl Expr {
    fn compute_constexpr(&self, context: &KoopaContext) -> i32 {
        match self {
            Expr::Number(n) => *n,
            Expr::Unary { op, expr } => {
                let val = expr.compute_constexpr(context);
                match op {
                    UnaryOp::Pos => val,
                    UnaryOp::Neg => -val,
                    // Note that `!val` is bitwise NOT instead of logical NOT
                    UnaryOp::Not => (val == 0) as i32,
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let left = lhs.compute_constexpr(context);
                let right = rhs.compute_constexpr(context);
                match op {
                    AstBinaryOp::Add => left + right,
                    AstBinaryOp::Sub => left - right,
                    AstBinaryOp::Mul => left * right,

                    // [TODO]: Check if right == 0
                    AstBinaryOp::Div => left / right,
                    AstBinaryOp::Mod => left % right,

                    AstBinaryOp::Eq => (left == right) as i32,
                    AstBinaryOp::Neq => (left != right) as i32,
                    AstBinaryOp::Lt => (left < right) as i32,
                    AstBinaryOp::Gt => (left > right) as i32,
                    AstBinaryOp::Leq => (left <= right) as i32,
                    AstBinaryOp::Geq => (left >= right) as i32,

                    AstBinaryOp::And => ((left != 0) && (right != 0)) as i32,
                    AstBinaryOp::Or => ((left != 0) || (right != 0)) as i32,
                }
            }
            Expr::LVal(name) => {
                let var_name = format!("@{}", name);
                let addr: VariableInfo = context
                    .symbol_table
                    .lookup(&var_name)
                    .expect("Variable not found in symbol table");
                let VariableInfo::ConstVariable(var) = addr else {
                    panic!("Cannot use non-constant variable in constant expression");
                };
                let v = context.get_value_kind(var);
                let ValueKind::Integer(n) = v else {
                    panic!("Constant variable does not hold an integer value");
                };
                n.value()
            }
        }
    }

    fn generate(&self, context: &mut KoopaContext) -> Value {
        match self {
            Expr::Number(n) => {
                context.new_value().integer(*n)
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
                        AstBinaryOp::And => KoopaBinaryOp::And,
                        AstBinaryOp::Or => KoopaBinaryOp::Or,
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
                let var_name = format!("@{}", name);
                let addr: VariableInfo = context
                    .symbol_table
                    .lookup(&var_name)
                    .expect("Variable not found in symbol table");
                match addr {
                    VariableInfo::ConstVariable(val) => val,
                    VariableInfo::Variable(val) => {
                        let load_inst = context.new_value().load(val);
                        context.add_inst(load_inst);
                        load_inst
                    }
                }
            }
        }
    }
}

fn map_binary_op(op: AstBinaryOp) -> Option<KoopaBinaryOp> {
    match op {
        AstBinaryOp::Add => Some(KoopaBinaryOp::Add),
        AstBinaryOp::Sub => Some(KoopaBinaryOp::Sub),
        AstBinaryOp::Mul => Some(KoopaBinaryOp::Mul),
        AstBinaryOp::Div => Some(KoopaBinaryOp::Div),
        AstBinaryOp::Mod => Some(KoopaBinaryOp::Mod),
        AstBinaryOp::Eq => Some(KoopaBinaryOp::Eq),
        AstBinaryOp::Neq => Some(KoopaBinaryOp::NotEq),
        AstBinaryOp::Lt => Some(KoopaBinaryOp::Lt),
        AstBinaryOp::Gt => Some(KoopaBinaryOp::Gt),
        AstBinaryOp::Leq => Some(KoopaBinaryOp::Le),
        AstBinaryOp::Geq => Some(KoopaBinaryOp::Ge),
        // And/Or are handled separately in the main logic
        AstBinaryOp::And | AstBinaryOp::Or => None,
    }
}
