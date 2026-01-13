# 编译原理实践：SysYrc

```bash
# 使用双斜杠 // 开头可以告诉 Shell：“这是一个绝对路径，请不要把它转换成 Windows 路径”
docker run -it --rm -v "D:/wksp/compilers/SysYrc":"//root/compiler" maxxing/compiler-dev autotest -riscv -s lv8 //root/compiler


# 启动 docker 容器，挂载项目目录到容器内
docker run -it --rm -v "D:/wksp/compilers/SysYrc":"//root/compiler" maxxing/compiler-dev bash
autotest -koopa -s lv1 /root/compiler
cd /opt/bin/testcases/lv9/    # 查看测试用例

cargo run -- -koopa hello.c -o hello.s
cargo run -- -riscv hello.c -o hello.o

```

cargo 的版本解析似乎有 bug，如果 rustc 版本不够新，cargo 会错误地解析出与 rustc 不兼容的依赖版本，导致构建失败（也就是求解 rustc 版本约束下的依赖时漏解了）

如果先复制一份 work 的 Cargo.toml 和 Cargo.lock 构建一次，然后再把 Cargo.toml 改成原本的样子，就能成功构建（我猜测是因为从干净基础上构建时会因为漏解而失败，而这个办法使得后一次构建时能使用上次构建时的 cache，从而绕过了漏解的求解过程）

# Koopa IR

Koopa IR 中, 最大的单位是 Program, 它由若干全局变量 Value 和 函数 Function 构成.
Function 由基本块 Basic Block 构成.
基本块中是一系列指令, 指令也是 Value.
- Program
    - Value 1
    - Value 2
    - ...
    - Function 1
        - Basic Block 1
            - Value 1
            - Value 2
            - ...
        - Basic Block 2
        - ...
    - Function 2
    - ...
基本块是一系列指令的集合, 它只有一个入口点且只有一个出口点. 即, 跳转的目标只能是基本块的开头, 且只有最后一条指令能进行控制流的转移

Value 的种类 (ValueKind) 有：
- 各类常量：Integer, ZeroInit, Undef, Aggregate
- 函数参数引用：FuncArgRef, 
- 块参数引用：BlockArgRef
- 内存分配：Alloc, GlobalAlloc
- 加载：Load, 存储：Store, 获取指针：GetPtr, 获取元素指针：GetElemPtr, 
- 二元运算：Binary, 分支：Branch, 跳转：Jump, 调用：Call, 返回：Return

Function, Basic Block, Value 的名字必须以 @ 或者 % 开头. 前者表示这是一个 "具名符号", 后者表示这是一个 "临时符号".

基本上，每个数据结构都分为 handle (指令 ID) 和 data (指令数据) 两部分，主要是为了方便所有权管理。
- 编译器 IR 是一个高度互联的图：指令需要引用它的操作数（其他指令），基本块包含指令列表，指令又需要知道它所属的基本块，...
- 通过分离 handle 和 data（Arena 模式），可以将借用推迟到运行时
  - Handle 是 Copy 类型，可以随意传递
  - 所有的 data 被存在一个 Vec 或 HashMap 里（称为 Pool 或 Arena），通过 handle 索引

## Program

Program
- IR 的根结点，拥有编译单元的所有资源，销毁它会释放所有资源
- 包含全局变量 handle 列表 `inst_layout`（数据通过 `borrow_values` 获取）
- 包含函数 handle 列表 `func_layout`（数据通过 `funcs` 获取）

```rust
impl Program {
    pub fn new() -> Self

    pub fn new_func(&mut self, data: FunctionData) -> Function
    pub fn remove_func(&mut self, func: Function) -> Option<FunctionData>

    // Returns a reference to the function map.
    pub fn funcs(&self) -> &HashMap<Function, FunctionData>    // Mutable version also exists.
    // Returns a reference to the layout of all functions.
    pub fn func_layout(&self) -> &[Function]
    pub fn func(&self, func: Function) -> &FunctionData    // Mutable version also exists.


    pub fn new_value(&mut self) -> GlobalBuilder<'_>
    // example usage:
    prog.new_value().integer(num);

    // Removes the given global value by its handle. Returns the corresponding value data.
    // Panics if the given value does not exist, or the removed value is currently used by other values.
    pub fn remove_value(&mut self, value: Value) -> ValueData
    pub fn set_value_name(&mut self, value: Value, name: Option<String>)

    // Returns a reference to the global value map.
    pub fn values(&self) -> &HashMap<Value, ValueData>    // Mutable version also exists.
    // Returns a reference to the layout of all global values.
    pub fn inst_layout(&self) -> &[Value]

    // Immutably borrows the global value map.
    pub fn borrow_values(&self) -> Ref<'_, HashMap<Value, ValueData>>
    // Immutably borrows the global value data by the given value handle.
    // Panics if the given value does not exist.
    pub fn borrow_value(&self, value: Value) -> Ref<'_, ValueData>

}
```

## Function

FunctionData includes a DFG and a Layout
- DFG (DataFlowGraph) holds all data of values (ValueData) and basicblocks (BasicBlockData), and maintains their use-define and define-use chain.
- Layout maintains the order of instructions (Value) and basic blocks (BasicBlock) in a function.
```rust
impl FunctionData {
    // Creates a new function definition.
    pub fn new(name: String, params_ty: Vec<Type>, ret_ty: Type) -> Self
    pub fn with_param_names(name: String, params: Vec<(Option<String>, Type)>, ret_ty: Type) -> Self
    // Creates a new function declaration.
    pub fn new_decl(name: String, params_ty: Vec<Type>, ret_ty: Type) -> Self

    pub fn ty(&self) -> &Type
    pub fn name(&self) -> &str
    pub fn set_name(&mut self, name: String)
    pub fn params(&self) -> &[Value]

    pub fn dfg(&self) -> &DataFlowGraph    // Mutable version also exists.
    pub fn layout(&self) -> &Layout    // Mutable version also exists.
}
```

Layout:
```rust
impl Layout {
    pub fn new() -> Self

    pub fn bbs(&self) -> &BasicBlockList    // Mutable version also exists.

    pub fn bb_mut(&mut self, bb: BasicBlock) -> &mut BasicBlockNode

    // Returns the entry basic block of the function, returns None if the function is a declaration.
    pub fn entry_bb(&self) -> Option<BasicBlock>
    // Returns the parent basic block of the given instruction, returns None if the given instruction is not in the current layout.
    pub fn parent_bb(&self, inst: Value) -> Option<BasicBlock>
}
```

在 FunctionData 中的 DFG 中，可以通过 BasicBlock 来获取 BasicBlockData；通过 Value 来获取 ValueData。
```rust
impl DataFlowGraph {
    pub fn new_value(&mut self) -> LocalBuilder<'_>
    pub fn replace_value_with(&mut self, value: Value) -> ReplaceBuilder<'_>
    pub fn remove_value(&mut self, value: Value) -> ValueData
    pub fn set_value_name(&mut self, value: Value, name: Option<String>)

    pub fn value(&self, value: Value) -> &ValueData
    pub fn values(&self) -> &HashMap<Value, ValueData>

    pub fn value_eq(&self, lhs: Value, rhs: Value) -> bool
    pub fn data_eq(&self, lhs: &ValueData, rhs: &ValueData) -> bool

    pub fn new_bb(&mut self) -> BlockBuilder<'_>

    // Removes the given basic block, also removes all basic block parameters. Returns the corresponding basic block data.
    pub fn remove_bb(&mut self, bb: BasicBlock) -> BasicBlockData

    pub fn bb(&self, bb: BasicBlock) -> &BasicBlockData    // Mutable version also exists.
    pub fn bbs(&self) -> &HashMap<BasicBlock, BasicBlockData>    // Mutable version also exists.
}
```


## BasicBlock

BasicBlockData 只包含该基本块的元信息，以及使用该基本块的 Value 集合。其余信息保存在对应 FunctionData 的 DFG 或 Layout 中。
```rust
impl BasicBlockData {
    pub fn name(&self) -> &Option<String>
    pub fn set_name(&mut self, name: Option<String>)

    pub fn params(&self) -> &[Value]    // Mutable version also exists.

    pub fn used_by(&self) -> &HashSet<Value>
}
```

BasicBlockList 是一个 KeyNodeList，存储函数中所有基本块的顺序信息。
```rust
pub type BasicBlockList = KeyNodeList<BasicBlock, BasicBlockNode, BasicBlockMap>;
```

BasicBlockNode 存储基本块内指令的顺序信息。
```rust
impl BasicBlockNode {
    pub fn insts(&self) -> &InstList    // Mutable version also exists.
}
// InstList 也是一个 KeyNodeList
pub type InstList = KeyNodeList<Value, InstNode, InstMap>;
// 添加指令：push_key_front, push_key_back, insert_key_before, insert_key_after
```

BasicBlockBuilder：
```rust
/// Returned by method DataFlowGraph::new_bb.
impl BasicBlockBuilder for BlockBuilder<'_> {
    fn basic_block(self, name: Option<String>) -> BasicBlock
    fn basic_block_with_param_names(
        self,
        name: Option<String>,
        params: Vec<(Option<String>, Type)>,
    ) -> BasicBlock
    fn basic_block_with_params(
        self,
        name: Option<String>,
        params_ty: Vec<Type>,
    ) -> BasicBlock
    fn insert_bb(&mut self, data: BasicBlockData) -> BasicBlock

}
```

## Value

Value 和 ValueData:
```rust
impl Value {
    pub fn is_global(&self) -> bool
}

impl ValueData {
    pub fn ty(&self) -> &Type
    pub fn name(&self) -> &Option<String>
    pub fn kind(&self) -> &ValueKind    // Mutable version also exists.
    // Returns a reference to the set of values that use this value.
    pub fn used_by(&self) -> &HashSet<Value>
}

impl LocalInstBuilder for LocalBuilder<'_> {
    fn alloc(self, ty: Type) -> Value
    fn load(self, src: Value) -> Value
    fn store(self, value: Value, dest: Value) -> Value
    fn get_ptr(self, src: Value, index: Value) -> Value
    fn get_elem_ptr(self, src: Value, index: Value) -> Value
    fn binary(self, op: BinaryOp, lhs: Value, rhs: Value) -> Value
    fn branch(self, cond: Value, true_bb: BasicBlock, false_bb: BasicBlock) -> Value
    fn branch_with_args(
        self,
        cond: Value,
        true_bb: BasicBlock,
        false_bb: BasicBlock,
        true_args: Vec<Value>,
        false_args: Vec<Value>,
    ) -> Value
    fn jump(self, target: BasicBlock) -> Value
    fn jump_with_args(self, target: BasicBlock, args: Vec<Value>) -> Value
    fn call(self, callee: Function, args: Vec<Value>) -> Value
    fn ret(self, value: Option<Value>) -> Value
}

impl ValueBuilder for LocalBuilder<'_> {
    fn raw(self, data: ValueData) -> Value
    fn integer(self, value: i32) -> Value
    fn zero_init(self, ty: Type) -> Value
    fn undef(self, ty: Type) -> Value
    fn aggregate(self, elems: Vec<Value>) -> Value
}kookoo
```

# Develop 

[TODO]: 
- Error handling
- Better argument parsing
- Buffered read and write?
- Instruction Set Structure
- Function-level Streaming

return 语句貌似支持无 expr 的形式。但是因为目前只有 int 类型的函数，这个功能尚未被测试到。

很坑的测试点: multiple returns
```c
int main() {
  return 5;
  return 4;
  return 3;
}
```

```asm
%after_ret_5:
  jump %end_2

%end_2:
  这种情况由于 jump 的存在，需要在 %end_2 处补充指令
  如果没有 jump，则不需要补充
```

```c
int main() {
    if (1) {
        return 4 + 5;
    } else {
        if (0) {
            return 2;
        } else {
            return 3;
        }
    }
}
// 猜测 13_branch2 应形如以上代码，SysYrc 生成的 Koopa IR 是
fun @main(): i32 {
%entry_0:
  br 1, %then_1, %else_3

%then_1:
  %0 = add 4, 5
  ret %0

%else_3:
  br 0, %then_4, %else_6

%then_4:
  ret 2

%else_6:
  ret 3

%end_5:
  jump %end_2    // 这里导致错误

%end_2:
}
```

解决方法：为 int 返回类型函数添加 default return 0


检查一下符号和符号表，尤其是函数参数的处理。

function call 只应该检查全局符号表？

```rust
// C:\Users\lenovo\.cargo\registry\src\index.crates.io-6f17d22bba15001f\koopa-0.0.8\src\back\koopa.rs

  /// Generates the given value.
  fn visit_value(&mut self, value: Value) -> Result<()> {
    if value.is_global() {
      let value = self.program.borrow_value(value);
      assert!(!value.kind().is_const());    // 这里的 assert 莫名奇妙！导致全局常量无法被正常生成！
      write!(self.w, "{}", self.nm.value_name(&value))
    } else {
      let value = value!(self, value);
      if value.kind().is_const() {
        self.visit_local_const(value)
      } else {
        write!(self.w, "{}", self.nm.value_name(value))
      }
    }
  }
```
