use crate::backend::riscv_context::*;
use koopa::ir::entities::ValueData;
use koopa::ir::{values::BinaryOp as KoopaBinaryOp, *};
use std::fmt;

/// Trait for generating RISC-V code from Koopa IR entities
/// The lifetime parameter 'a ensures that any references
/// within the ctx remain valid during the generation process
pub trait GenerateRiscv {
    // fmt::Result is an alias for Result<(), fmt::Error>
    fn generate<'a>(&'a self, ctx: &mut RiscvContext<'a>) -> fmt::Result;
}

impl GenerateRiscv for Program {
    fn generate<'a>(&'a self, ctx: &mut RiscvContext<'a>) -> fmt::Result {
        ctx.program = self;

        ctx.write_inst(format_args!(".data"))?;
        for &global in self.inst_layout() {
            let global_data = self.borrow_value(global);
            let name = global_data.name().as_ref().unwrap().replace("@", "");
            ctx.write_inst(format_args!(".globl {}", name))?;
            ctx.write_line(&format!("{}:", name))?;
            match global_data.kind() {
                ValueKind::GlobalAlloc(alloc) => {
                    let init = alloc.init();
                    let init_data = self.borrow_value(init);
                    match init_data.kind() {
                        ValueKind::Integer(int) => {
                            ctx.write_inst(format_args!(".word {}", int.value()))?;
                        }
                        ValueKind::ZeroInit(_) => {
                            ctx.write_inst(format_args!(".zero {}", WORD_SIZE))?;
                        }
                        _ => {
                            panic!("Unsupported global initializer");
                        }
                    }
                }
                _ => {
                    panic!("Unsupported global value kind");
                }
            }
        }

        for &func in self.func_layout() {
            let func_data = self.func(func);
            // Skip function declarations (none entry basic block)
            if func_data.layout().entry_bb().is_none() {
                continue;
            }
            ctx.current_func = Some(func);
            func_data.generate(ctx)?;
        }
        Ok(())
    }
}

impl GenerateRiscv for FunctionData {
    fn generate<'a>(&'a self, ctx: &mut RiscvContext<'a>) -> fmt::Result {
        // Function name starts with an '@'
        let name = self.name().replace("@", "");
        ctx.write_inst(format_args!(".text"))?;
        ctx.write_inst(format_args!(".globl {}", name))?;
        ctx.write_line(&format!("{}:", name))?;

        // Stack frame setup
        ctx.init_stack_frame();
        ctx.generate_prologue()?;
        ctx.save_caller_saved_regs()?;

        // Generate code for each basic block
        for (&bb, node) in self.layout().bbs() {
            // node is a &BasicBlockNode
            let bb_name = ctx.get_bb_name(bb);
            ctx.write_line(&format!("{}:", bb_name))?;

            // Generate code for each instruction in the basic block
            for &inst in node.insts().keys() {
                // inst is a Value
                let inst_data = self.dfg().value(inst);
                ctx.current_value = Some(inst);
                inst_data.generate(ctx)?;
            }
        }
        Ok(())
    }
}

impl GenerateRiscv for ValueData {
    fn generate<'a>(&'a self, ctx: &mut RiscvContext<'a>) -> fmt::Result {
        match self.kind() {
            ValueKind::Integer(_) => {}

            ValueKind::Call(call) => {
                let args = call.args();

                // Load arguments into regs or stack
                for (i, &arg) in args.iter().enumerate() {
                    if i < 8 {
                        ctx.load_value_to_reg(arg, &format!("a{}", i))?;
                    } else {
                        ctx.load_value_to_reg(arg, "t0")?;
                        let offset = (i as i32 - 8) * WORD_SIZE;
                        ctx.prepare_addr(offset, "t1")?;
                        let addr = ctx.get_addr_str(offset, "t1");
                        ctx.write_inst(format_args!("sw t0, {}", addr))?;
                    }
                }

                // Call the function
                let callee = call.callee();
                let callee_name = ctx.program.func(callee).name().replace("@", "");
                ctx.write_inst(format_args!("call {}", callee_name))?;

                // Save return value if there is one
                if !self.ty().is_unit() {
                    ctx.save_value_from_reg(ctx.current_value.unwrap(), "a0")?;
                }
            }

            ValueKind::Return(value) => {
                // Load return value into a0 if exists
                if let Some(ret_value) = value.value() {
                    ctx.load_value_to_reg(ret_value, "a0")?;
                }
                ctx.restore_caller_saved_regs()?;
                ctx.generate_epilogue()?;
                ctx.write_inst(format_args!("ret\n"))?;
            }

            ValueKind::Binary(bin) => {
                ctx.load_value_to_reg(bin.lhs(), "t0")?;
                ctx.load_value_to_reg(bin.rhs(), "t1")?;

                let op_str = map_binary_op(bin.op());
                match bin.op() {
                    KoopaBinaryOp::Le => {
                        ctx.write_inst(format_args!("sgt t0, t0, t1"))?; // t0 = (lhs > rhs)
                        ctx.write_inst(format_args!("seqz t0, t0"))?; // t0 = (t0 == 0) => !(lhs > rhs) => lhs <= rhs
                    }
                    KoopaBinaryOp::Ge => {
                        ctx.write_inst(format_args!("slt t0, t0, t1"))?;
                        ctx.write_inst(format_args!("seqz t0, t0"))?;
                    }
                    KoopaBinaryOp::Eq => {
                        ctx.write_inst(format_args!("xor t0, t0, t1"))?;
                        ctx.write_inst(format_args!("seqz t0, t0"))?;
                    }
                    KoopaBinaryOp::NotEq => {
                        ctx.write_inst(format_args!("xor t0, t0, t1"))?;
                        ctx.write_inst(format_args!("snez t0, t0"))?;
                    }
                    _ => {
                        // Regular binary operations
                        if let Some(op) = op_str {
                            ctx.write_inst(format_args!("{} t0, t0, t1", op))?;
                        } else {
                            unreachable!("Unknown binary op");
                        }
                    }
                }
                ctx.save_value_from_reg(ctx.current_value.unwrap(), "t0")?;
            }

            ValueKind::Alloc(_) => {
                // Allocation handled in stack frame setup.
                // Does nothing here
            }

            ValueKind::Store(store) => {
                let value = store.value();
                let dest = store.dest();
                if dest.is_global() {
                    let global_name = ctx
                        .program
                        .borrow_value(dest)
                        .name()
                        .as_ref()
                        .unwrap()
                        .replace("@", "");
                    ctx.load_value_to_reg(value, "t0")?;
                    ctx.write_inst(format_args!("la t1, {}", global_name))?;
                    ctx.write_inst(format_args!("sw t0, 0(t1)"))?;
                    return Ok(());
                }
                let offset = ctx.get_stack_offset(dest);
                ctx.load_value_to_reg(value, "t0")?;
                ctx.prepare_addr(offset, "t1")?;
                let addr: String = ctx.get_addr_str(offset, "t1");
                ctx.write_inst(format_args!("sw t0, {}", addr))?;
            }

            ValueKind::Load(load) => {
                let src = load.src();
                if src.is_global() {
                    let global_name = ctx
                        .program
                        .borrow_value(src)
                        .name()
                        .as_ref()
                        .unwrap()
                        .replace("@", "");
                    ctx.write_inst(format_args!("la t0, {}", global_name))?;
                    ctx.write_inst(format_args!("lw t0, 0(t0)"))?;
                    ctx.save_value_from_reg(ctx.current_value.unwrap(), "t0")?;
                } else {
                    let offset = ctx.get_stack_offset(src);

                    ctx.prepare_addr(offset, "t0")?;
                    let addr: String = ctx.get_addr_str(offset, "t0");
                    ctx.write_inst(format_args!("lw t0, {}", addr))?;

                    ctx.save_value_from_reg(ctx.current_value.unwrap(), "t0")?;
                }
            }

            ValueKind::Branch(branch) => {
                let cond = branch.cond();
                let true_bb = branch.true_bb();
                let false_bb = branch.false_bb();

                ctx.load_value_to_reg(cond, "t0")?;
                let true_bb_name = ctx.get_bb_name(true_bb);
                let false_bb_name = ctx.get_bb_name(false_bb);
                ctx.write_inst(format_args!("bnez t0, {}", true_bb_name))?;
                ctx.write_inst(format_args!("j {}", false_bb_name))?;
            }

            ValueKind::Jump(jump) => {
                let target_bb = jump.target();
                let target_bb_name = ctx.get_bb_name(target_bb);
                ctx.write_inst(format_args!("j {}", target_bb_name))?;
            }

            _ => {
                panic!("Unsupported instruction in RISC-V generation");
            }
        }
        Ok(())
    }
}

fn map_binary_op(op: KoopaBinaryOp) -> Option<&'static str> {
    match op {
        // All instructions are in the format `op rd, rs1, rs2`
        KoopaBinaryOp::Add => Some("add"),
        KoopaBinaryOp::Sub => Some("sub"),
        KoopaBinaryOp::Mul => Some("mul"),
        KoopaBinaryOp::Div => Some("div"),
        KoopaBinaryOp::Mod => Some("rem"),
        KoopaBinaryOp::And => Some("and"),
        KoopaBinaryOp::Or => Some("or"),
        KoopaBinaryOp::Lt => Some("slt"),
        KoopaBinaryOp::Gt => Some("sgt"),
        KoopaBinaryOp::Sar => Some("sra"),
        KoopaBinaryOp::Shl => Some("sll"),
        KoopaBinaryOp::Shr => Some("srl"),
        KoopaBinaryOp::Xor => Some("xor"),
        KoopaBinaryOp::Eq | KoopaBinaryOp::NotEq | KoopaBinaryOp::Ge | KoopaBinaryOp::Le => None,
    }
}
