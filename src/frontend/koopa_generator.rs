use crate::ast::{BinaryOp as AstBinaryOp, *};
use crate::frontend::{
    array_init_helper::*, koopa_context::KoopaContext, symbol_table::SymbolInfo,
};
use koopa::ir::{builder_traits::*, values::BinaryOp as KoopaBinaryOp, *};

/// Trait for generating Koopa IR entities
pub trait GenerateKoopa {
    fn generate(&self, ctx: &mut KoopaContext) -> ();
}

impl GenerateKoopa for CompUnit {
    fn generate(&self, ctx: &mut KoopaContext) -> () {
        // Register all SysY library functions
        ctx.register_sysy_lib_functions();

        for item in &self.items {
            match item {
                GlobalItem::Decl(decl) => decl.generate(ctx),
                GlobalItem::FuncDef(func_def) => func_def.generate(ctx),
            }
        }
    }
}

impl GenerateKoopa for FuncDef {
    fn generate(&self, ctx: &mut KoopaContext) -> () {
        // Create and register the function
        let func_params_config: Vec<_> = self
            .params
            .iter()
            .map(|param| {
                let ty = match param.param_type {
                    DataType::Int => Type::get_i32(),
                };
                let name = format!("@{}", param.param_name);
                (Some(name), ty)
            })
            .collect(); // Vector of (param_name, param_type) tuples
        let ret_type = match self.func_type {
            FuncType::Int => Type::get_i32(),
            FuncType::Void => Type::get_unit(),
        };
        let func_data = FunctionData::with_param_names(
            format!("@{}", self.func_name),
            func_params_config.clone(),
            ret_type,
        );
        let func = ctx.program.new_func(func_data);
        ctx.set_current_func(func);
        // Insert the function into global symbol table
        ctx.symbol_table
            .insert(self.func_name.clone(), SymbolInfo::Function(func));

        // Create entry basic block
        let entry_bb: BasicBlock = ctx.new_bb("%entry");
        ctx.add_bb(entry_bb);
        ctx.set_current_bb(entry_bb);

        // Set up stack arguments: alloc & store
        ctx.symbol_table.enter_scope(); // Enter function scope
        for (i, arg) in self.params.iter().enumerate() {
            let value: Value = ctx.current_func().params()[i];

            let ty = match arg.param_type {
                DataType::Int => Type::get_i32(),
            };
            let name = format!("%{}", arg.param_name);

            let alloc_inst = ctx.new_value().alloc(ty);
            ctx.set_value_name(alloc_inst, name.clone());
            ctx.add_inst(alloc_inst);

            let store_inst = ctx.new_value().store(value, alloc_inst);
            ctx.add_inst(store_inst);

            ctx.symbol_table
                .insert(arg.param_name.clone(), SymbolInfo::Variable(alloc_inst));
        }

        // Generate function body
        self.block.generate(ctx);

        // Default return if no return statement is present
        if !ctx.is_current_bb_terminated() {
            let ret_value = match self.func_type {
                FuncType::Int => {
                    let zero = ctx.new_value().integer(0);
                    Some(zero)
                }
                FuncType::Void => None,
            };
            let ret_inst = ctx.new_value().ret(ret_value);
            ctx.add_inst(ret_inst);
        }
        ctx.symbol_table.exit_scope(); // Exit function scope
    }
}

impl GenerateKoopa for Block {
    fn generate(&self, ctx: &mut KoopaContext) -> () {
        for item in &self.items {
            if ctx.is_current_bb_terminated() {
                // Dead code elimination: stop generating further instructions
                // e.g., return 1; return 2; <- the second return is dead code
                break;
            }
            match item {
                BlockItem::Stmt(stmt) => stmt.generate(ctx),
                BlockItem::Decl(decl) => decl.generate(ctx),
            }
        }
    }
}

impl GenerateKoopa for Decl {
    fn generate(&self, ctx: &mut KoopaContext) -> () {
        match self {
            Decl::Const {
                var_type,
                var_name,
                init_list,
            } => {
                let init_expr = unwrap_init_list(init_list);

                let init_value: i32 = init_expr.compute_constexpr(ctx);
                let init_handle = if ctx.symbol_table.is_global_scope() {
                    ctx.new_global_value().integer(init_value)
                } else {
                    ctx.new_value().integer(init_value)
                };
                ctx.symbol_table
                    .insert(var_name.clone(), SymbolInfo::ConstVariable(init_handle));
            }
            Decl::Var {
                var_type,
                var_name,
                init_list,
            } => {
                let init_expr = if let Some(init_list) = init_list {
                    Some(unwrap_init_list(init_list))
                } else {
                    None
                };
                let var_type = match var_type {
                    DataType::Int => Type::get_i32(),
                };
                // Global variable
                if ctx.symbol_table.is_global_scope() {
                    let init = if let Some(expr) = init_expr {
                        // Initializer for global variables must be a constexpr
                        let init_value = expr.compute_constexpr(ctx);
                        ctx.new_global_value().integer(init_value)
                    } else {
                        // Default initialize to zero
                        ctx.new_global_value().zero_init(var_type)
                    };
                    let alloc_ptr = ctx.new_global_value().global_alloc(init);
                    // No need to append scope level to global variable names
                    ctx.set_value_name(alloc_ptr, format!("@{}", var_name));
                    ctx.symbol_table
                        .insert(var_name.clone(), SymbolInfo::Variable(alloc_ptr));
                    return;
                }
                // Local variable
                let alloc_ptr = ctx.new_value().alloc(var_type);
                // Koopa IR value names must be unique
                let unique_name = format!("@{}_{}", var_name, ctx.symbol_table.level());
                ctx.set_value_name(alloc_ptr, unique_name);

                ctx.add_inst(alloc_ptr);
                // If there is an initializer, calculate and store the value
                if let Some(expr) = init_expr {
                    let expr_value = expr.generate(ctx);
                    let store_inst = ctx.new_value().store(expr_value, alloc_ptr);
                    ctx.add_inst(store_inst);
                }
                ctx.symbol_table
                    .insert(var_name.clone(), SymbolInfo::Variable(alloc_ptr));
            }
            Decl::ConstArray {
                var_type,
                var_name,
                dims,
                init_list,
            }
            | Decl::Array {
                var_type,
                var_name,
                dims,
                init_list,
            } => {
                let shape: Vec<usize> = dims
                    .iter()
                    .map(|dim_expr| dim_expr.compute_constexpr(ctx) as usize)
                    .collect();
                let elem_type = match var_type {
                    DataType::Int => Type::get_i32(),
                };
                // Initialization for local arrays: getelemptr + store
                let array_type = build_array_type(elem_type.clone(), &shape);
                if ctx.symbol_table.is_global_scope() {
                    // Global array
                    let init = if let Some(_init_list) = init_list {
                        let mut helper = ArrayInitHelper::new(ctx, &shape);
                        let flat_vals = helper.flatten_init_list(init_list);
                        helper.generate_global_init(&flat_vals)
                    } else {
                        // Default initialize to zero
                        ctx.new_global_value().zero_init(array_type.clone())
                    };

                    let alloc_ptr = ctx.new_global_value().global_alloc(init);
                    // No need to append scope level to global variable names
                    ctx.set_value_name(alloc_ptr, format!("@{}", var_name));
                    ctx.symbol_table
                        .insert(var_name.clone(), SymbolInfo::Variable(alloc_ptr));
                } else {
                    // Local array
                    let alloc_ptr = ctx.new_value().alloc(array_type.clone());
                    // Koopa IR value names must be unique
                    let unique_name = format!("@{}_{}", var_name, ctx.symbol_table.level());
                    ctx.set_value_name(alloc_ptr, unique_name);
                    ctx.add_inst(alloc_ptr);

                    // If there is an initializer, calculate and store the values
                    if let Some(_init_list) = init_list {
                        let mut helper = ArrayInitHelper::new(ctx, &shape);
                        let flat_vals = helper.flatten_init_list(init_list);
                        helper.generate_local_init(alloc_ptr, &flat_vals);
                    }
                    ctx.symbol_table
                        .insert(var_name.clone(), SymbolInfo::Variable(alloc_ptr));
                }
            }
        };
    }
}

impl GenerateKoopa for Stmt {
    fn generate(&self, ctx: &mut KoopaContext) -> () {
        match self {
            Stmt::Return { expr } => {
                if let Some(expr) = expr {
                    let value: Value = expr.generate(ctx);
                    let inst: Value = ctx.new_value().ret(Some(value));
                    ctx.add_inst(inst);
                }
            } // Stmt::Return
            Stmt::Assign { lval, expr } => {
                let addr: SymbolInfo = ctx
                    .symbol_table
                    .lookup(lval)
                    .expect(&format!("Variable {} not found in symbol table", lval));
                match addr {
                    SymbolInfo::ConstVariable(_) => {
                        panic!("Cannot assign to a constant variable");
                    }
                    SymbolInfo::Variable(var_addr) => {
                        let expr_value = expr.generate(ctx);
                        let store_inst = ctx.new_value().store(expr_value, var_addr);
                        ctx.add_inst(store_inst);
                    }
                    SymbolInfo::Function(_) => {
                        unreachable!()
                    }
                }
            } // Stmt::Assign
            Stmt::Expression { expr } => {
                if let Some(expr) = expr {
                    let _ = expr.generate(ctx);
                }
            } // Stmt::Expression
            Stmt::Block { block } => {
                ctx.symbol_table.enter_scope();
                block.generate(ctx);
                ctx.symbol_table.exit_scope();
            } // Stmt::Block
            Stmt::If {
                cond,
                then_body,
                else_body,
            } => {
                // If (cond) then { ... } else { ... }
                // will be translated to:
                // cond calcalation
                // br cond, then_bb, else_bb
                // then_bb:
                //   then_body
                //   jump end_bb
                // else_bb:
                //   else_body
                //   jump end_bb
                // end_bb:
                //   ...

                // Special case: no else
                // If (cond) then { ... }
                // will be translated to:
                // cond calcalation
                // br cond, then_bb, end_bb
                // then_bb:
                //   then_body
                //   jump end_bb
                // end_bb:
                //   ...
                let cond_value = cond.generate(ctx);
                let has_else: bool = else_body.is_some();
                let then_bb = ctx.new_bb("%then");
                let end_bb = ctx.new_bb("%end");
                let else_bb = if has_else {
                    ctx.new_bb("%else")
                } else {
                    end_bb
                };

                let branch_inst = ctx.new_value().branch(
                    cond_value, then_bb, else_bb, // If no else body, jump to end_bb directly
                );
                ctx.add_inst(branch_inst);

                // Then body
                ctx.add_bb(then_bb);
                ctx.set_current_bb(then_bb);
                then_body.generate(ctx);
                // Check if then_bb already ends with a jump/branch/ret
                // If not, we need to add a jump to the end_bb
                // The only case then_bb is terminated is when then_body ends
                // with a return statement
                if !ctx.is_current_bb_terminated() {
                    let jump_to_merge_from_then = ctx.new_value().jump(end_bb);
                    ctx.add_inst(jump_to_merge_from_then);
                }
                // Else body
                if let Some(else_body) = else_body {
                    ctx.add_bb(else_bb);
                    ctx.set_current_bb(else_bb);
                    else_body.generate(ctx);
                    // It is necessary to jump to the end block after else body
                    // even if they are adjacent, because Koopa IR basic blocks
                    // must end with ret/branch/jump instructions
                    if !ctx.is_current_bb_terminated() {
                        let jump_to_merge_from_else = ctx.new_value().jump(end_bb);
                        ctx.add_inst(jump_to_merge_from_else);
                    }
                }

                // End block
                ctx.add_bb(end_bb);
                ctx.set_current_bb(end_bb);
            } // Stmt::If
            Stmt::While { cond, body } => {
                // while (cond) { body }
                // will be translated to:
                // jump cond_bb
                // cond_bb:
                //   cond calculation
                //   br cond, body_bb, end_bb
                // body_bb:
                //   body
                //   jump cond_bb
                // end_bb:
                //   ...

                let cond_bb = ctx.new_bb("%while_cond");
                let body_bb = ctx.new_bb("%while_body");
                let end_bb = ctx.new_bb("%while_end");

                // Initial jump to condition check
                let initial_jump = ctx.new_value().jump(cond_bb);
                ctx.add_inst(initial_jump);

                // Condition block
                ctx.add_bb(cond_bb);
                ctx.set_current_bb(cond_bb);
                let cond_value = cond.generate(ctx);
                let branch_inst = ctx.new_value().branch(cond_value, body_bb, end_bb);
                ctx.add_inst(branch_inst);

                // Body block
                ctx.add_bb(body_bb);
                ctx.set_current_bb(body_bb);
                // Push information for break/continue statements before
                // generating the loop body
                ctx.enter_loop(end_bb, cond_bb);
                body.generate(ctx);
                ctx.exit_loop();
                // After body, jump back to condition check
                // The only case current_bb is terminated is when body ends
                // with a return statement
                if !ctx.is_current_bb_terminated() {
                    let jump_to_cond = ctx.new_value().jump(cond_bb);
                    ctx.add_inst(jump_to_cond);
                }

                // End block
                ctx.add_bb(end_bb);
                ctx.set_current_bb(end_bb);
            } // Stmt::While
            Stmt::Break => {
                let target = ctx.get_current_loop_break_target();
                let jump_inst = ctx.new_value().jump(target);
                ctx.add_inst(jump_inst);
            } // Stmt::Break
            Stmt::Continue => {
                let target = ctx.get_current_loop_continue_target();
                let jump_inst = ctx.new_value().jump(target);
                ctx.add_inst(jump_inst);
            } // Stmt::Continue
        }
    }
}

impl Expr {
    fn compute_constexpr(&self, ctx: &KoopaContext) -> i32 {
        match self {
            Expr::Number(n) => *n,
            Expr::Unary { op, expr } => {
                let val = expr.compute_constexpr(ctx);
                match op {
                    UnaryOp::Pos => val,
                    UnaryOp::Neg => -val,
                    // Note that `!val` is bitwise NOT instead of logical NOT
                    UnaryOp::Not => (val == 0) as i32,
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let left = lhs.compute_constexpr(ctx);
                let right = rhs.compute_constexpr(ctx);
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
            // Constant variables are also treated as LVal here
            Expr::LVal(name) => {
                let addr: SymbolInfo = ctx
                    .symbol_table
                    .lookup(name)
                    .expect(&format!("Variable {} not found in symbol table", name));
                let SymbolInfo::ConstVariable(var) = addr else {
                    panic!("Cannot use non-constant variable in constant expression");
                };
                let v = ctx.get_value_kind(var);
                let ValueKind::Integer(n) = v else {
                    panic!("Constant variable does not hold an integer value");
                };
                n.value()
            }
            Expr::Call {
                func_name: _,
                args: _,
            } => {
                panic!("Constant variable cannot have function calls");
            }
        }
    }

    fn generate(&self, ctx: &mut KoopaContext) -> Value {
        match self {
            Expr::Number(n) => ctx.new_value().integer(*n),
            Expr::Binary { op, lhs, rhs } => {
                let lhs_value = lhs.generate(ctx);

                match op {
                    AstBinaryOp::And => {
                        // Short-circuiting Logical AND (&&)
                        // Logic: result = 0; if (lhs != 0) { result = (rhs != 0); }

                        // Allocate temporary variable for result, default to 0 (False)
                        let result_ptr = ctx.new_value().alloc(Type::get_i32());
                        ctx.add_inst(result_ptr);
                        let zero = ctx.new_value().integer(0);
                        let store_zero = ctx.new_value().store(zero, result_ptr);
                        ctx.add_inst(store_zero);

                        // Check if LHS is true
                        let lhs_ne_zero =
                            ctx.new_value()
                                .binary(KoopaBinaryOp::NotEq, lhs_value, zero);
                        ctx.add_inst(lhs_ne_zero);

                        // Create basic blocks
                        let eval_rhs_bb = ctx.new_bb("%and_eval_rhs"); // For evaluating the right-hand side
                        let end_bb = ctx.new_bb("%and_end"); // End and merge

                        // Branch: if LHS is true, evaluate RHS; otherwise go directly to End (result remains 0)
                        let branch = ctx.new_value().branch(lhs_ne_zero, eval_rhs_bb, end_bb);
                        ctx.add_inst(branch);

                        // RHS evaluation block
                        ctx.add_bb(eval_rhs_bb);
                        ctx.set_current_bb(eval_rhs_bb);

                        let rhs_value = rhs.generate(ctx);
                        let rhs_ne_zero =
                            ctx.new_value()
                                .binary(KoopaBinaryOp::NotEq, rhs_value, zero);
                        ctx.add_inst(rhs_ne_zero);
                        let store_rhs = ctx.new_value().store(rhs_ne_zero, result_ptr);
                        ctx.add_inst(store_rhs);
                        let jump = ctx.new_value().jump(end_bb);
                        ctx.add_inst(jump);

                        // End block
                        ctx.add_bb(end_bb);
                        ctx.set_current_bb(end_bb);
                        let result = ctx.new_value().load(result_ptr);
                        ctx.add_inst(result);
                        result
                    } // AstBinaryOp::And

                    AstBinaryOp::Or => {
                        // Short-circuiting Logical OR (||)
                        // Logic: result = 1; if (lhs == 0) { result = (rhs != 0); }

                        // Allocate temporary variable for result, default to 1 (True)
                        let result_ptr = ctx.new_value().alloc(Type::get_i32());
                        ctx.add_inst(result_ptr);

                        let one = ctx.new_value().integer(1);
                        let store_one = ctx.new_value().store(one, result_ptr);
                        ctx.add_inst(store_one);

                        // Check if LHS is true
                        let zero = ctx.new_value().integer(0);
                        let lhs_ne_zero =
                            ctx.new_value()
                                .binary(KoopaBinaryOp::NotEq, lhs_value, zero);
                        ctx.add_inst(lhs_ne_zero);

                        // Create basic blocks
                        let eval_rhs_bb = ctx.new_bb("%or_eval_rhs");
                        let end_bb = ctx.new_bb("%or_end");

                        // Branch: if LHS is true, go directly to End (short-circuit, result is 1); otherwise evaluate RHS
                        let branch = ctx.new_value().branch(lhs_ne_zero, end_bb, eval_rhs_bb);
                        ctx.add_inst(branch);

                        // RHS evaluation block
                        ctx.add_bb(eval_rhs_bb);
                        ctx.set_current_bb(eval_rhs_bb);
                        let rhs_value = rhs.generate(ctx);
                        let rhs_ne_zero =
                            ctx.new_value()
                                .binary(KoopaBinaryOp::NotEq, rhs_value, zero);
                        ctx.add_inst(rhs_ne_zero);
                        let store_rhs = ctx.new_value().store(rhs_ne_zero, result_ptr);
                        ctx.add_inst(store_rhs);
                        let jump = ctx.new_value().jump(end_bb);
                        ctx.add_inst(jump);

                        // End block
                        ctx.add_bb(end_bb);
                        ctx.set_current_bb(end_bb);
                        let result = ctx.new_value().load(result_ptr);
                        ctx.add_inst(result);
                        result
                    } // AstBinaryOp::Or

                    _ => {
                        // Normal binary operations (Add, Sub, Eq, ...)
                        let rhs_value = rhs.generate(ctx);

                        if let Some(koopa_op) = map_binary_op(*op) {
                            let inst = ctx.new_value().binary(koopa_op, lhs_value, rhs_value);
                            ctx.add_inst(inst);
                            inst
                        } else {
                            panic!("Unknown binary operator");
                        }
                    }
                } // match op
            } // Expr::Binary
            Expr::Unary { op, expr } => match op {
                UnaryOp::Pos => expr.generate(ctx),
                UnaryOp::Neg => {
                    let value = expr.generate(ctx);
                    let zero = ctx.new_value().integer(0);
                    let inst = ctx.new_value().binary(KoopaBinaryOp::Sub, zero, value);
                    ctx.add_inst(inst);
                    inst
                }
                UnaryOp::Not => {
                    let value = expr.generate(ctx);
                    let zero = ctx.new_value().integer(0);
                    let inst = ctx.new_value().binary(KoopaBinaryOp::Eq, value, zero);
                    ctx.add_inst(inst);
                    inst
                }
            },

            Expr::LVal { name, indices } => {
                let addr: SymbolInfo = ctx
                    .symbol_table
                    .lookup(name)
                    .expect(&format!("Variable {} not found in symbol table", name));
                match addr {
                    SymbolInfo::ConstVariable(val) => {
                        // Koopa IR library does not allow global constant values
                        // to be operated directly, for I don't know why...
                        // This is a workaround to load the actual value into
                        // the local context
                        if val.is_global() {
                            let kind = ctx.get_value_kind(val);
                            let value = match kind {
                                ValueKind::Integer(value) => value,
                                _ => panic!("Constant global variable is not an integer"),
                            };
                            ctx.new_value().integer(value.value())
                        } else {
                            val
                        }
                    }
                    SymbolInfo::Variable(val) => {
                        let load_inst = ctx.new_value().load(val);
                        ctx.add_inst(load_inst);
                        load_inst
                    }
                    SymbolInfo::Function(_) => unreachable!(),
                }
            }

            Expr::Call { func_name, args } => {
                let symbol_info = ctx
                    .symbol_table
                    .lookup(func_name)
                    .expect("Function not found");
                let SymbolInfo::Function(func) = symbol_info else {
                    panic!("Symbol is not a function");
                };

                let mut arg_values = Vec::new();
                for arg in args {
                    arg_values.push(arg.generate(ctx));
                }

                let call_inst = ctx.new_value().call(func, arg_values);
                ctx.add_inst(call_inst);

                // `void` functions return a unit value
                call_inst
            }
        }
    }
}

/// Unwraps an InitList to get the contained Expr for variable declarations.
/// The InitList must contain exactly one expression
fn unwrap_init_list(init_list: &InitList) -> &Expr {
    match init_list {
        InitList::Expr(expr) => expr,
        InitList::List(ls) => {
            // Should contain exactly one expression
            if ls.len() != 1 {
                panic!("Variable declaration initializer list must contain exactly one expression");
            }
            match &ls[0] {
                InitList::Expr(expr) => expr,
                _ => panic!("Variable declaration initializer list must contain an expression"),
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
