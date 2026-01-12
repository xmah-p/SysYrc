use crate::frontend::koopa_context::KoopaContext;
use crate::ast::*;


/// Builds a Koopa array type bottom up
/// Input: base_type=i32, dims=[2, 3] -> 输出: [[i32, 3], 2]
pub fn build_array_type(base_type: Type, dims: &[usize]) -> Type {
    let mut current_type = base_type;
    for &dim in dims.iter().rev() {
        current_type = Type::get_array(current_type, dim);
    }
    current_type
}

/// Helper struct to handle array initialization logic
pub struct ArrayInitHelper<'a> {
    ctx: &'a mut KoopaContext,
    shape: &'a [usize], // Array dimensions [2, 3, 4]
    flat_size: usize,   // Total number of elements 24
}

impl<'a> ArrayInitHelper<'a> {
    pub fn new(ctx: &'a mut KoopaContext, shape: &'a [usize]) -> Self {
        let flat_size = shape.iter().product();
        Self {
            ctx,
            shape,
            flat_size,
        }
    }

    /// 核心逻辑：将 InitList 转换为线性的数值列表
    /// 如果是 Global，返回 Vec<i32> (常量)
    /// 如果是 Local，我们通常也需要先算出这个列表，如果是变量则生成 Load 指令
    /// 这里简化处理，假设我们能得到一个扁平的 `Vec<Option<Value>>`
    /// None 表示需要补零，Some 表示显式初始化
    pub fn flatten_init_list(&mut self, init: &Option<InitList>) -> Vec<Value> {
        // [TODO]: 这里需要实现那个复杂的递归填充算法 (Flatten + Padding)
        // 算法思路：
        // 1. 维护一个 cursor 指向当前扁平数组的索引
        // 2. 递归遍历 InitList
        // 3. 遇到 '{' 进入下一维，结束时 cursor 需要对齐到该维度的步长
        // 4. 返回一个长度为 self.flat_size 的 Vec，未填充部分填入 Zero (Const 0)

        // int arr[2][3][4] = {1, 2, 3, 4, {5}, {6}, {7, 8}};
        // shape = [2, 3, 4], flat_size = 24
        // flatten 结果应为：
        // [1, 2, 3, 4, 5, 0, 0, 0, 6, 0, 0, 0, 7, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        vec![self.ctx.new_value().integer(0); self.flat_size]
    }

    /// Generate aggregate initializer for global arrays
    pub fn generate_global_init(&mut self, flat_values: &[Value]) -> Value {
        // 全局初始值必须是 Aggregate 嵌套结构
        // 需要按照 shape 从后往前折叠 flat_values
        // Koopa API: ctx.new_global_value().aggregate(vec![...])
        // [TODO]: 实现 Aggregate 构建逻辑
        // 占位返回：
        self.ctx.new_global_value().zero_init(Type::get_i32())
    }

    /// Generate store instructions for local array initialization
    pub fn generate_local_init(&mut self, alloc_ptr: Value, flat_values: &[Value]) {
        for (i, &val) in flat_values.iter().enumerate() {
            // 优化：如果是 0 且数组已经是 alloc 出来的（默认未定义，但 Koopa 模拟器通常不清零）
            // SysY 标准要求局部数组初始化时，未显式初始化的部分补 0。
            // 所以即使是 0 也要 store，除非你用了 memset。

            // 1. 计算多维索引
            // 例如 shape=[2, 3], i=3 -> indices=[1, 0]
            let mut idx = i;
            let mut ptr = alloc_ptr;

            // 2. 生成 getelemptr 链
            for (dim_idx, &dim_size) in self.shape.iter().enumerate() {
                // 计算当前维度的跨度 (stride)
                let stride: usize = self.shape.iter().skip(dim_idx + 1).product();
                let current_idx = idx / stride;
                idx %= stride;

                let idx_val = self.ctx.new_value().integer(current_idx as i32);

                // 注意：第一维如果是指针(参数)用 getptr，如果是数组(alloc)用 getelemptr
                // 这里 alloc_ptr 是数组指针，所以全程 getelemptr
                ptr = self.ctx.new_value().get_elem_ptr(ptr, idx_val);
                self.ctx.add_inst(ptr);
            }

            // 3. Store
            let store = self.ctx.new_value().store(val, ptr);
            self.ctx.add_inst(store);
        }
    }
}
