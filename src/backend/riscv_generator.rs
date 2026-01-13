use crate::backend::asm_writer::AsmWriter;
use crate::backend::stack_frame::StackFrame;
use koopa::ir::entities::*;
use koopa::ir::{values::BinaryOp as KoopaBinaryOp, *};
use std::io::{self, Write};

pub const WORD_SIZE: i32 = 4;
const MAX_IMM_12: i32 = 2047; // Maximum positive immediate for 12-bit signed integer

pub struct RiscvGenerator<'a, W: Write> {
    program: &'a Program,
    writer: AsmWriter<W>,
}

impl<'a, W: Write> RiscvGenerator<'a, W> {
    pub fn new(program: &'a Program, writer: W) -> Self {
        Self {
            program,
            writer: AsmWriter::new(writer),
        }
    }

    pub fn generate_program(&mut self) -> io::Result<()> {
        let program = self.program;

        // Generate data segment for global variables
        self.writer.write_directive("data", &[])?;
        for &global in program.inst_layout() {
            let name = self.get_global_value_name(global);

            self.writer.write_directive("globl", &[&name])?;
            self.writer.write_label(&name)?;

            let kind = self.get_global_value_kind(global);
            let ValueKind::GlobalAlloc(alloc) = kind else {
                unreachable!("Expected GlobalAlloc for global variable");
            };
            // Initialization
            self.generate_global_init(alloc.init())?;
        }
        self.writer.write_blank_line()?;

        // Generate text segment for functions
        self.writer.write_directive("text", &[])?;
        for &func in program.func_layout() {
            let func_data = program.func(func);
            // Skip function declarations (none entry basic block)
            if func_data.layout().entry_bb().is_none() {
                continue;
            }
            let mut func_gen = FunctionGenerator::new(self, func_data);
            func_gen.generate_function()?;
        }
        Ok(())
    }

    fn generate_global_init(&mut self, init: Value) -> io::Result<()> {
        let kind = self.get_global_value_kind(init);
        let ty = self.get_global_value_type(init);
        match kind {
            ValueKind::Integer(int) => self
                .writer
                .write_directive("word", &[&int.value().to_string()]),
            ValueKind::ZeroInit(_) => self
                .writer
                .write_directive("zero", &[&ty.size().to_string()]),
            ValueKind::Aggregate(agg) => {
                for &elem in agg.elems() {
                    self.generate_global_init(elem)?;
                }
                Ok(())
            }
            _ => unreachable!("Unsupported global initializer"),
        }
    }

    fn get_global_value_name(&self, value: Value) -> String {
        self.program
            .borrow_value(value)
            .name()
            .as_ref()
            .unwrap()
            .replace("@", "")
    }

    fn get_global_value_kind(&self, value: Value) -> ValueKind {
        self.program.borrow_value(value).kind().clone()
    }

    fn get_global_value_type(&self, value: Value) -> Type {
        self.program.borrow_value(value).ty().clone()
    }
}

struct FunctionGenerator<'a, 'b, W: Write> {
    gen: &'a mut RiscvGenerator<'b, W>,
    func: &'b FunctionData,
    stack_frame: StackFrame,
}

impl<'a, 'b, W: Write> FunctionGenerator<'a, 'b, W> {
    pub fn new(riscv_gen: &'a mut RiscvGenerator<'b, W>, func: &'b FunctionData) -> Self {
        let mut stack_frame = StackFrame::new();
        stack_frame.initialize(func);
        Self {
            gen: riscv_gen,
            func,
            stack_frame,
        }
    }

    fn generate_function(&mut self) -> io::Result<()> {
        // Function name starts with an '@'
        let name = self.func.name().replace("@", "");
        self.gen.writer.write_directive("globl", &[&name])?;
        self.gen.writer.write_label(&name)?;

        // Stack frame setup
        self.generate_prologue()?;
        self.save_caller_saved_regs()?;

        // Generate code for each basic block
        let mut is_first_bb = true;
        for (&bb, node) in self.func.layout().bbs() {
            // node is a &BasicBlockNode
            let bb_name = self.get_bb_name(bb);
            // Skip label for the entry basic block (already labeled above)
            if !is_first_bb {
                self.gen.writer.write_label(&bb_name)?;
            } else {
                is_first_bb = false;
            }

            // Generate code for each instruction in the basic block
            for &inst in node.insts().keys() {
                self.generate_instruction(inst)?;
            }
        }
        Ok(())
    }

    fn generate_instruction(&mut self, value: Value) -> io::Result<()> {
        let value_kind = self.get_value_kind(value);
        match value_kind {
            ValueKind::Integer(_) => {}

            ValueKind::Call(call) => {
                let args = call.args();

                // Load arguments into regs or stack
                for (i, &arg) in args.iter().enumerate() {
                    if i < 8 {
                        self.load_value_to_reg(arg, &format!("a{}", i))?;
                    } else {
                        self.load_value_to_reg(arg, "t0")?;
                        let offset = (i as i32 - 8) * WORD_SIZE;
                        let addr = self.build_stk_addr_str(offset, "t1")?;
                        self.gen.writer.write_inst("sw", &["t0", &addr])?;
                    }
                }

                // Call the function
                let callee = call.callee();
                let callee_name = self.gen.program.func(callee).name().replace("@", "");
                self.gen.writer.write_inst("call", &[&callee_name])?;

                // Save return value if there is one
                let value_type = self.get_value_type(value);
                if !value_type.is_unit() {
                    self.save_value_from_reg(value, "a0")?;
                }
            }

            ValueKind::Return(ret) => {
                // Load return value into a0 if exists
                if let Some(ret_value) = ret.value() {
                    self.load_value_to_reg(ret_value, "a0")?;
                }
                self.restore_caller_saved_regs()?;
                self.generate_epilogue()?;
                self.gen.writer.write_inst("ret", &[])?;
                self.gen.writer.write_blank_line()?;
            }

            ValueKind::Binary(bin) => {
                self.load_value_to_reg(bin.lhs(), "t0")?;
                self.load_value_to_reg(bin.rhs(), "t1")?;

                let op_str = map_binary_op(bin.op());
                match bin.op() {
                    KoopaBinaryOp::Le => {
                        self.gen.writer.write_inst("sgt", &["t0", "t0", "t1"])?; // t0 = (lhs > rhs)
                        self.gen.writer.write_inst("seqz", &["t0", "t0"])?; // t0 = (t0 == 0) => !(lhs > rhs) => lhs <= rhs
                    }
                    KoopaBinaryOp::Ge => {
                        self.gen.writer.write_inst("slt", &["t0", "t0", "t1"])?;
                        self.gen.writer.write_inst("seqz", &["t0", "t0"])?;
                    }
                    KoopaBinaryOp::Eq => {
                        self.gen.writer.write_inst("xor", &["t0", "t0", "t1"])?;
                        self.gen.writer.write_inst("seqz", &["t0", "t0"])?;
                    }
                    KoopaBinaryOp::NotEq => {
                        self.gen.writer.write_inst("xor", &["t0", "t0", "t1"])?;
                        self.gen.writer.write_inst("snez", &["t0", "t0"])?;
                    }
                    _ => {
                        // Regular binary operations
                        if let Some(op) = op_str {
                            self.gen.writer.write_inst(op, &["t0", "t0", "t1"])?;
                        }
                    }
                }
                self.save_value_from_reg(value, "t0")?;
            }

            ValueKind::Alloc(_) => {
                // Allocation handled in stack frame setup.
                // Does nothing here
            }

            ValueKind::GetElemPtr(gep) => {
                let src = gep.src();
                let index = gep.index();
                let src_type = self.get_value_type(src);
                let step = match src_type.kind() {
                    TypeKind::Pointer(base) => match base.kind() {
                        TypeKind::Array(elem, _) => elem.size(),
                        _ => base.size(),
                    },
                    _ => panic!("GetElemPtr src must be a pointer"),
                };
                self.generate_ptr_calc(value, src, index, step)?;
            }

            ValueKind::GetPtr(gp) => {
                let src = gp.src();
                let index = gp.index();
                let src_type = self.get_value_type(src);

                let step = match src_type.kind() {
                    TypeKind::Pointer(base) => base.size(),
                    _ => panic!("GetPtr src must be a pointer"),
                };
                self.generate_ptr_calc(value, src, index, step)?;
            }

            ValueKind::Store(store) => {
                let store_value = store.value();
                let dest = store.dest();
                self.load_value_to_reg(store_value, "t0")?;
                if dest.is_global() {
                    let global_name = self.gen.get_global_value_name(dest);
                    self.gen.writer.write_inst("la", &["t1", &global_name])?;
                    self.gen.writer.write_inst("sw", &["t0", "0(t1)"])?;
                } else {
                    let offset = self.stack_frame.get_stack_offset(dest);
                    let addr: String = self.build_stk_addr_str(offset, "t1")?;
                    self.gen.writer.write_inst("sw", &["t0", &addr])?;
                }
            }

            ValueKind::Load(load) => {
                let src = load.src();
                if src.is_global() {
                    let global_name = self.gen.get_global_value_name(src);
                    self.gen.writer.write_inst("la", &["t0", &global_name])?;
                    self.gen.writer.write_inst("lw", &["t0", "0(t0)"])?;
                } else {
                    let offset = self.stack_frame.get_stack_offset(src);
                    let addr: String = self.build_stk_addr_str(offset, "t0")?;
                    self.gen.writer.write_inst("lw", &["t0", &addr])?;
                }
                self.save_value_from_reg(value, "t0")?;
            }

            ValueKind::Branch(branch) => {
                let cond = branch.cond();
                let true_bb = branch.true_bb();
                let false_bb = branch.false_bb();

                self.load_value_to_reg(cond, "t0")?;
                let true_bb_name = self.get_bb_name(true_bb);
                let false_bb_name = self.get_bb_name(false_bb);
                self.gen.writer.write_inst("bnez", &["t0", &true_bb_name])?;
                self.gen.writer.write_inst("j", &[&false_bb_name])?;
            }

            ValueKind::Jump(jump) => {
                let target_bb = jump.target();
                let target_bb_name = self.get_bb_name(target_bb);
                self.gen.writer.write_inst("j", &[&target_bb_name])?;
            }

            _ => {
                panic!("Unsupported instruction in RISC-V generation");
            }
        }
        Ok(())
    }

    /// Generate pointer calculation for GetElemPtr and GetPtr
    /// dest = src + index * step
    fn generate_ptr_calc(
        &mut self,
        dest: Value,
        src: Value,
        index: Value,
        step: usize,
    ) -> io::Result<()> {
        self.load_value_to_reg(src, "t0")?;
        self.load_value_to_reg(index, "t1")?;

        if step != 1 {
            if step.is_power_of_two() {
                let shift = step.trailing_zeros();
                self.gen
                    .writer
                    .write_inst("slli", &["t1", "t1", &shift.to_string()])?;
            } else {
                self.gen
                    .writer
                    .write_inst("li", &["t2", &step.to_string()])?;
                self.gen.writer.write_inst("mul", &["t1", "t1", "t2"])?;
            }
        }
        self.gen.writer.write_inst("add", &["t0", "t0", "t1"])?;
        self.save_value_from_reg(dest, "t0")
    }

    /// Allocate space for the current function's stack frame by adjusting
    /// `sp`. `t0` is used as a temporary register if the offset exceeds
    /// the 12-bit immediate limit.
    fn generate_prologue(&mut self) -> io::Result<()> {
        let stack_size = self.stack_frame.get_stack_size();
        if stack_size == 0 {
            return Ok(());
        }
        let offset = (-stack_size).to_string();
        if stack_size > MAX_IMM_12 {
            self.gen.writer.write_inst("li", &["t0", &offset])?;
            self.gen.writer.write_inst("add", &["sp", "sp", "t0"])?;
        } else {
            self.gen.writer.write_inst("addi", &["sp", "sp", &offset])?;
        }
        Ok(())
    }

    /// Deallocate space for the current function's stack frame by adjusting
    /// `sp`. `t0` is used as a temporary register if the offset exceeds
    /// the 12-bit immediate limit.
    fn generate_epilogue(&mut self) -> io::Result<()> {
        let stack_size = self.stack_frame.get_stack_size();
        if stack_size == 0 {
            return Ok(());
        }
        let offset = stack_size.to_string();
        if stack_size > MAX_IMM_12 {
            self.gen.writer.write_inst("li", &["t0", &offset])?;
            self.gen.writer.write_inst("add", &["sp", "sp", "t0"])?;
        } else {
            self.gen.writer.write_inst("addi", &["sp", "sp", &offset])?;
        }
        Ok(())
    }

    /// Builds a stack address string for RISC-V load/store instructions.
    /// If the offset fits in a 12-bit immediate, it returns "offset(sp)"
    /// Otherwise, it loads the offset into a temporary register and returns "0(tmp_reg)"
    fn build_stk_addr_str(&mut self, offset: i32, tmp_reg: &str) -> io::Result<String> {
        if offset <= MAX_IMM_12 && offset >= -MAX_IMM_12 - 1 {
            return Ok(format!("{}(sp)", offset));
        }
        self.gen
            .writer
            .write_inst("li", &["t0", &offset.to_string()])?;
        self.gen.writer.write_inst("add", &[tmp_reg, "sp", "t0"])?;
        Ok(format!("0({})", tmp_reg))
    }

    /// Save caller-saved registers (currently only `ra`) onto the stack
    fn save_caller_saved_regs(&mut self) -> io::Result<()> {
        let Some(ra_offset) = self.stack_frame.get_ra_offset() else {
            return Ok(());
        };
        let addr = self.build_stk_addr_str(ra_offset, "t0")?;
        self.gen.writer.write_inst("sw", &["ra", &addr])
    }

    /// Restore caller-saved registers (currently only `ra`) from the stack
    fn restore_caller_saved_regs(&mut self) -> io::Result<()> {
        let Some(ra_offset) = self.stack_frame.get_ra_offset() else {
            return Ok(());
        };
        let addr = self.build_stk_addr_str(ra_offset, "t0")?;
        self.gen.writer.write_inst("lw", &["ra", &addr])
    }

    /// Load a value (global or local) into a register
    fn load_value_to_reg(&mut self, value: Value, reg: &str) -> io::Result<()> {
        if value.is_global() {
            // [TODO] buggy for arrays?
            let global_name = self.gen.get_global_value_name(value);
            return self.gen.writer.write_inst("la", &[reg, &global_name]);
        }
        // Non-global values reside in the stack frame
        let kind = self.get_value_kind(value);
        match kind {
            ValueKind::Integer(int) => {
                if int.value() == 0 {
                    self.gen.writer.write_inst("mv", &[reg, "x0"])
                } else {
                    self.gen
                        .writer
                        .write_inst("li", &[reg, &int.value().to_string()])
                }
            }
            ValueKind::FuncArgRef(arg) => {
                let arg_index = arg.index() as i32;
                if arg_index < 8 {
                    self.gen
                        .writer
                        .write_inst("mv", &[reg, &format!("a{}", arg_index)])
                } else {
                    let offset = (arg_index - 8) * WORD_SIZE + self.stack_frame.get_stack_size();
                    let addr: String = self.build_stk_addr_str(offset, reg)?;
                    self.gen.writer.write_inst("lw", &[reg, &addr])
                }
            }
            // Result of other instructions
            // They should have been already stored on the stack
            _ => {
                let offset = self.stack_frame.get_stack_offset(value);
                let addr: String = self.build_stk_addr_str(offset, "t0")?;
                self.gen.writer.write_inst("lw", &[reg, &addr])
            }
        }
    }

    fn save_value_from_reg(&mut self, value: Value, reg: &str) -> io::Result<()> {
        if value.is_global() {
            let global_name = self.gen.get_global_value_name(value);
            self.gen.writer.write_inst("la", &["t0", &global_name])?;
            return self.gen.writer.write_inst("sw", &[reg, "0(t0)"]);
        }
        let offset = self.stack_frame.get_stack_offset(value);
        let addr: String = self.build_stk_addr_str(offset, "t0")?;
        self.gen.writer.write_inst("sw", &[reg, &addr])
    }

    fn get_value_kind(&self, value: Value) -> ValueKind {
        self.func.dfg().value(value).kind().clone()
    }

    fn get_value_type(&self, value: Value) -> Type {
        self.func.dfg().value(value).ty().clone()
    }

    fn get_bb_name(&self, bb: BasicBlock) -> String {
        self.func
            .dfg()
            .bb(bb)
            .name()
            .as_ref()
            .unwrap()
            .replace("%", "")
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
