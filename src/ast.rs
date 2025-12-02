// Abstract Syntax Tree (AST) definitions for SysY language

#[derive(Debug)]
pub struct CompUnit {
    pub func_def: FuncDef,
}

#[derive(Debug)]
pub struct FuncDef {
    pub func_type: FuncType,
    pub identifier: String,
    pub block: Block,
}

#[derive(Debug)]
pub enum FuncType {
    Int,
}

#[derive(Debug)]
pub struct Block {
    pub stmt: Stmt,
}

#[derive(Debug)]
pub struct Stmt {
    pub expr: Expr,
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
    Number(i32),
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum UnaryOp {
    Pos,
    Neg,
    Not,
}