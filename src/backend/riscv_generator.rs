use crate::backend::riscv_context::RiscvContext;
use koopa::ir::entities::ValueData;
use koopa::ir::{values::BinaryOp as KoopaBinaryOp, *};
use std::fmt;

/// Trait for generating RISC-V code from Koopa IR entities
/// The lifetime parameter 'a ensures that any references
/// within the context remain valid during the generation process
pub trait GenerateRiscv {
    // fmt::Result is an alias for Result<(), fmt::Error>
    fn generate<'a>(&'a self, context: &mut RiscvContext<'a>) -> fmt::Result;
}

impl GenerateRiscv for Program {
    fn generate<'a>(&'a self, context: &mut RiscvContext<'a>) -> fmt::Result {
        context.program = Some(self);
        context.write_line(".text")?;
        context.write_line(".globl main")?;

        for &func in self.func_layout() {
            let func_data: &FunctionData = self.func(func);
            context.current_func = Some(func);
            func_data.generate(context)?;
            context.current_func = None;
        }
        Ok(())
    }
}

impl GenerateRiscv for FunctionData {
    fn generate<'a>(&'a self, context: &mut RiscvContext<'a>) -> fmt::Result {
        // Function name starts with an '@'
        let name = self.name().replace("@", "");
        context.write_line(&format!("{}:", name))?;

        // Stack frame setup
        context.init_stack_frame();
        context.generate_prologue()?;

        // Generate code for each basic block
        for (&bb, node) in self.layout().bbs() {
            // node is a &BasicBlockNode
            let bb_name = self.dfg().bb(bb).name().as_ref().unwrap().replace("%", "");
            context.write_line(&format!("{}:", bb_name))?;

            // Generate code for each instruction in the basic block
            for &inst in node.insts().keys() {
                // inst is a Value
                let inst_data = self.dfg().value(inst);
                context.current_value = Some(inst);
                inst_data.generate(context)?;
            }
        }
        Ok(())
    }
}

impl GenerateRiscv for ValueData {
    fn generate<'a>(&'a self, context: &mut RiscvContext<'a>) -> fmt::Result {
        match self.kind() {
            ValueKind::Integer(_) => {}

            ValueKind::Return(value) => {
                let Some(ret_value) = value.value() else {
                    panic!("Unsupported return instruction without value");
                };

                context.load_value_to_reg(ret_value, "a0")?;
                context.generate_epilogue()?;
                context.write_inst(format_args!("ret"))?;
            }

            ValueKind::Binary(bin) => {
                context.load_value_to_reg(bin.lhs(), "t0")?;
                context.load_value_to_reg(bin.rhs(), "t1")?;

                let op_str = map_binary_op(bin.op());
                match bin.op() {
                    KoopaBinaryOp::Le => {
                        context.write_inst(format_args!("sgt t0, t0, t1"))?; // t0 = (lhs > rhs)
                        context.write_inst(format_args!("seqz t0, t0"))?; // t0 = (t0 == 0) => !(lhs > rhs) => lhs <= rhs
                    }
                    KoopaBinaryOp::Ge => {
                        context.write_inst(format_args!("slt t0, t0, t1"))?;
                        context.write_inst(format_args!("seqz t0, t0"))?;
                    }
                    KoopaBinaryOp::Eq => {
                        context.write_inst(format_args!("xor t0, t0, t1"))?;
                        context.write_inst(format_args!("seqz t0, t0"))?;
                    }
                    KoopaBinaryOp::NotEq => {
                        context.write_inst(format_args!("xor t0, t0, t1"))?;
                        context.write_inst(format_args!("snez t0, t0"))?;
                    }
                    _ => {
                        // Regular binary operations
                        if let Some(op) = op_str {
                            context.write_inst(format_args!("{} t0, t0, t1", op))?;
                        } else {
                            unreachable!("Unknown binary op");
                        }
                    }
                }
                context.save_value_to_reg(context.current_value.unwrap(), "t0")?;
            }

            ValueKind::Alloc(_) => {
                // Allocation handled in stack frame setup.
                // Does nothing here
            }

            ValueKind::Store(store) => {
                let value = store.value();
                let dest = store.dest();
                let offset = context.get_stack_offset(dest);
                context.load_value_to_reg(value, "t0")?;
                context.prepare_addr(offset, "t1")?;
                let addr: String = context.get_addr_str(offset, "t1");
                context.write_inst(format_args!("sw t0, {}", addr))?;
            }

            ValueKind::Load(load) => {
                let src = load.src();
                let offset = context.get_stack_offset(src);

                context.prepare_addr(offset, "t0")?;
                let addr: String = context.get_addr_str(offset, "t0");
                context.write_inst(format_args!("lw t0, {}", addr))?;

                context.save_value_to_reg(context.current_value.unwrap(), "t0")?;
            }

            ValueKind::Branch(branch) => {
                let cond = branch.cond();
                let true_bb = branch.true_bb();
                let false_bb = branch.false_bb();

                context.load_value_to_reg(cond, "t0")?;
                let true_bb_name = context.get_bb_name(true_bb);
                let false_bb_name = context.get_bb_name(false_bb);
                context.write_inst(format_args!(
                    "bnez t0, {}",
                    true_bb_name
                ))?;
                context.write_inst(format_args!("j {}", false_bb_name))?;
            }

            ValueKind::Jump(jump) => {
                let target_bb = jump.target();
                let target_bb_name = context.get_bb_name(target_bb);
                context.write_inst(format_args!("j {}", target_bb_name))?;
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
