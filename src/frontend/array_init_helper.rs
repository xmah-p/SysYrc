use crate::ast::*;
use crate::frontend::koopa_context::KoopaContext;
use koopa::ir::{builder_traits::*, *};

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
pub struct ArrayInitHelper<'init, 'ctx> {
    ctx: &'init mut KoopaContext<'ctx>,
    shape: &'init [usize], // Array dimensions [2, 3, 4]
    flat_size: usize,      // Total number of elements 24
}

impl<'init, 'ctx> ArrayInitHelper<'init, 'ctx> {
    pub fn new(ctx: &'init mut KoopaContext<'ctx>, shape: &'init [usize]) -> Self {
        let flat_size = shape.iter().product();
        Self {
            ctx,
            shape,
            flat_size,
        }
    }

    pub fn flatten_init_list(&mut self, init: &Option<InitList>) -> Vec<Value> {
        let zero_val = if self.ctx.symbol_table.is_global_scope() {
            self.ctx.new_global_value().integer(0)
        } else {
            self.ctx.new_value().integer(0)
        };
        let mut result = vec![zero_val; self.flat_size];
        let mut cursor = 0;
        if let Some(init_list) = init {
            self.flatten_recursive(init_list, 0, &mut cursor, &mut result);
        }
        result
    }

    /// Recursively flatten the InitList into a flat vector of Values
    /// current_dim: current dimension in shape:
    ///   - 0 means outside the array
    ///   - 1 means first dimension
    ///   - ...
    fn flatten_recursive(
        &mut self,
        current_init: &InitList,
        current_dim: usize,
        cursor: &mut usize,
        result: &mut Vec<Value>,
    ) {
        match current_init {
            InitList::Expr(expr) => {
                if *cursor >= result.len() {
                    return;
                }
                let val = if self.ctx.symbol_table.is_global_scope() {
                    let int_val = expr.compute_constexpr(self.ctx);
                    self.ctx.new_global_value().integer(int_val)
                } else {
                    expr.generate(self.ctx)
                };

                result[*cursor] = val;
                *cursor += 1;
            }
            InitList::List(list) => {
                let start_cursor = *cursor;

                for item in list {
                    match item {
                        InitList::List(_) => {
                            let mut next_dim = current_dim + 1;
                            // int[2][3][4]
                            // next_dim = 1 -> capacity = 2 * 3 * 4 = 24
                            // next_dim = 2 -> capacity = 3 * 4 = 12
                            // next_dim = 3 -> capacity = 4
                            // next_dim = 4 -> panic
                            loop {
                                let next_capacity: usize =
                                    self.shape.iter().skip(next_dim - 1).product();
                                if *cursor % next_capacity == 0 {
                                    break;
                                }
                                next_dim += 1;
                                if next_dim > self.shape.len() {
                                    println!("cursor: {}, shape: {:?}", cursor, self.shape);

                                    panic!(
                                        "ArrayInitHelper: cannot align cursor for nested init list"
                                    );
                                }
                            }

                            self.flatten_recursive(item, next_dim, cursor, result)
                        }
                        InitList::Expr(_) => {
                            self.flatten_recursive(item, current_dim, cursor, result)
                        }
                    }
                }

                if current_dim == 0 {
                    return;
                }
                let capacity: usize = self.shape.iter().skip(current_dim - 1).product();

                let end_cursor = start_cursor + capacity;
                if *cursor < end_cursor {
                    *cursor = end_cursor;
                }
            }
        }
    }

    /// Generate aggregate initializer for global arrays
    pub fn generate_global_init(&mut self, flat_values: Vec<Value>) -> Value {
        if flat_values.is_empty() {
            return self.ctx.new_global_value().zero_init(Type::get_i32()); // Should not happen given logic
        }

        let mut current_level_values = flat_values;

        // shape: [2, 3, 4]
        // 1. chunks(4) -> aggregate -> new values (size 2*3)
        // 2. chunks(3) -> aggregate -> new values (size 2)
        // 3. chunks(2) -> aggregate -> new values (size 1)
        for &dim_size in self.shape.iter().rev() {
            let mut next_level_values = Vec::new();

            for chunk in current_level_values.chunks(dim_size) {
                let agg = self.ctx.new_global_value().aggregate(chunk.to_vec());
                next_level_values.push(agg);
            }

            current_level_values = next_level_values;
        }

        current_level_values[0]
    }

    /// Generate store instructions for local array initialization
    pub fn generate_local_init(&mut self, alloc_ptr: Value, flat_values: &[Value]) {
        for (i, &val) in flat_values.iter().enumerate() {
            let mut idx = i;
            let mut ptr = alloc_ptr;

            for (dim_idx, &_dim_size) in self.shape.iter().enumerate() {
                let stride: usize = self.shape.iter().skip(dim_idx + 1).product();
                let current_idx = idx / stride;
                idx %= stride;

                let idx_val = self.ctx.new_value().integer(current_idx as i32);

                ptr = self.ctx.new_value().get_elem_ptr(ptr, idx_val);
                self.ctx.add_inst(ptr);
            }

            let store = self.ctx.new_value().store(val, ptr);
            self.ctx.add_inst(store);
        }
    }
}
