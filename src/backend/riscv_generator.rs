use crate::backend::riscv_context::RiscvContext;
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

        for &func in self.func_layout() {
            let func_data = self.func(func);
            ctx.current_func = Some(func);
            func_data.generate(ctx)?;
        }
        Ok(())
    }
}

/// Stack frame layout:
/// 
/// Stack frame for previous function
/// Saved ra
/// Local variables...
/// 10th argument
/// 9th argument
/// Stack frame for Next function
impl GenerateRiscv for FunctionData {
    fn generate<'a>(&'a self, ctx: &mut RiscvContext<'a>) -> fmt::Result {
        // Function name starts with an '@'
        let name = self.name().replace("@", "");
        ctx.write_line(".text")?;
        ctx.write_line(&format!(".globl {}", name))?;
        ctx.write_line(&format!("{}:", name))?;

        // Stack frame setup
        ctx.init_stack_frame();
        ctx.generate_prologue()?;

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

            ValueKind::Return(value) => {
                let Some(ret_value) = value.value() else {
                    panic!("Unsupported return instruction without value");
                };

                ctx.load_value_to_reg(ret_value, "a0")?;
                ctx.generate_epilogue()?;
                ctx.write_inst(format_args!("ret"))?;
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
                ctx.save_value_to_reg(ctx.current_value.unwrap(), "t0")?;
            }

            ValueKind::Alloc(_) => {
                // Allocation handled in stack frame setup.
                // Does nothing here
            }

            ValueKind::Store(store) => {
                let value = store.value();
                let dest = store.dest();
                let offset = ctx.get_stack_offset(dest);
                ctx.load_value_to_reg(value, "t0")?;
                ctx.prepare_addr(offset, "t1")?;
                let addr: String = ctx.get_addr_str(offset, "t1");
                ctx.write_inst(format_args!("sw t0, {}", addr))?;
            }

            ValueKind::Load(load) => {
                let src = load.src();
                let offset = ctx.get_stack_offset(src);

                ctx.prepare_addr(offset, "t0")?;
                let addr: String = ctx.get_addr_str(offset, "t0");
                ctx.write_inst(format_args!("lw t0, {}", addr))?;

                ctx.save_value_to_reg(ctx.current_value.unwrap(), "t0")?;
            }

            ValueKind::Branch(branch) => {
                let cond = branch.cond();
                let true_bb = branch.true_bb();
                let false_bb = branch.false_bb();

                ctx.load_value_to_reg(cond, "t0")?;
                let true_bb_name = ctx.get_bb_name(true_bb);
                let false_bb_name = ctx.get_bb_name(false_bb);
                ctx.write_inst(format_args!(
                    "bnez t0, {}",
                    true_bb_name
                ))?;
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
