use super::context::RiscvContext;
use koopa::ir::entities::ValueData;
use koopa::ir::values::BinaryOp as KoopaBinaryOp;
use koopa::ir::{FunctionData, Program, ValueKind};
use std::fmt;

/// Trait for generating RISC-V code from Koopa IR entities
/// The lifetime parameter 'a ensures that any references
/// within the context remain valid during the generation process
pub trait GenerateRiscv {
    // fmt::Result is an alias for Result<(), fmt::Error>
    // Lifetime parameter 'a ensures that the context
    // lives at least as long as the data being generated
    fn generate<'a>(&'a self, context: &mut RiscvContext<'a>) -> fmt::Result;
}

impl GenerateRiscv for Program {
    fn generate<'a>(&'a self, context: &mut RiscvContext<'a>) -> fmt::Result {
        context.program = Some(self);
        context.write_inst(".text")?;
        context.write_inst(".globl main")?;

        for &func in self.func_layout() {
            let func_data: &FunctionData = self.func(func);
            context.current_func = Some(func_data);
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
        context.init_stack_frame();
        let stack_size = context.get_stack_size();
        if stack_size > 0 {
            // [TODO]: Handle large stack sizes that exceed immediate range
            context.write_inst(&format!("addi sp, sp, -{}", stack_size))?;
        }

        for (&bb, node) in self.layout().bbs() {
            // node is a BasicBlockNode
            let bb_name = self.dfg().bb(bb).name().as_ref().unwrap().replace("%", "");
            context.write_line(&format!("{}:", bb_name))?;

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
                    // error
                    panic!("Unsupported return instruction without value");
                };
                context.load_value_to_reg(ret_value, "a0")?;
                // Function epilogue
                let stack_size = context.get_stack_size();
                if stack_size > 0 {
                    // [TODO]: Handle large stack sizes that exceed immediate range
                    context.write_inst(&format!("addi sp, sp, {}", stack_size))?;
                }
                context.write_inst("ret")?;
            }

            ValueKind::Binary(bin) => {
                context.load_value_to_reg(bin.lhs(), "t0")?;
                context.load_value_to_reg(bin.rhs(), "t1")?;

                let op_str = map_binary_op(bin.op());
                if let Some(op) = op_str {
                    match op {
                        "seqz" => context.write_inst(&format!("{} t0, t0", op))?,
                        "snez" => context.write_inst(&format!("{} t0, t0", op))?,
                        _ => context.write_inst(&format!("{} t0, t0, t1", op))?,
                    }
                } else {
                    // Handle le and ge
                    match bin.op() {
                        KoopaBinaryOp::Le => {
                            context.write_inst("slt t0, t1, t0")?;
                            context.write_inst("xori t0, t0, 1")?;
                        }
                        KoopaBinaryOp::Ge => {
                            context.write_inst("slt t0, t0, t1")?;
                            context.write_inst("xori t0, t0, 1")?;
                        }
                        _ => {
                            panic!("Unsupported binary operation in RISC-V generation");
                        }
                    }
                }
                context.save_value_to_reg(context.current_value.unwrap(), "t0")?;
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
        // Except for seqz and snez, the rest are in the form:
        // op rd, rs1, rs2
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
        KoopaBinaryOp::Eq => Some("seqz"),    // seqz rd, rs1
        KoopaBinaryOp::NotEq => Some("snez"), // snez rd, rs1
        KoopaBinaryOp::Ge | KoopaBinaryOp::Le | _ => None,
    }
}
