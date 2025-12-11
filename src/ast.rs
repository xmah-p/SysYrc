// Abstract Syntax Tree (AST) definitions for SysY language

#[derive(Debug)]
pub struct CompUnit {
    pub func_def: FuncDef,
}

#[derive(Debug)]
pub struct FuncDef {
    pub func_type: FuncType,
    pub func_name: String,
    pub block: Block,
}

#[derive(Debug)]
pub struct Block {
    pub items: Vec<BlockItem>,
}

#[derive(Debug)]
pub enum BlockItem {
    Decl(Decl),
    Stmt(Stmt),
}

#[derive(Debug)]
pub struct Decl {
    pub constant: bool,
    pub var_type: ValueType,
    pub var_name: String,
    pub init_expr: Option<Expr>,
}

#[derive(Debug)]
pub enum Stmt {
    Return { expr: Expr },
    Assign {
        lval: String,
        expr: Expr,
    },
}

#[derive(Debug)]
pub enum Expr {
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    // Note that constant variable references are also treated as LVal here
    // Their values will be resolved during constant expression evaluation
    LVal(String),
    Number(i32),
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryOp {
    Or,
    And,
    Eq,
    Neq,
    Lt,
    Gt,
    Leq,
    Geq,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Copy, Clone)]
pub enum UnaryOp {
    Pos,
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy)]
pub enum FuncType {
    Int,
}

#[derive(Debug, Clone, Copy)]
pub enum ValueType {
    Int,
}
