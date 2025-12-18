// Abstract Syntax Tree (AST) definitions for SysY language

#[derive(Debug, Clone, Copy)]
pub enum DataType {
    Int,
}

#[derive(Debug, Clone, Copy)]
pub enum FuncType {
    Void,
    Int,
}

#[derive(Debug)]
pub enum GlobalItem {
    Decl(Decl),
    FuncDef(FuncDef),
}

#[derive(Debug)]
pub struct CompUnit {
    pub items: Vec<GlobalItem>,
}

#[derive(Debug)]
pub struct FuncDef {
    pub func_type: FuncType,
    pub func_name: String,
    pub params: Vec<FuncFParam>,
    pub block: Block,
}

#[derive(Debug)]
pub struct FuncFParam {
    pub param_type: DataType,
    pub param_name: String,
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
    pub var_type: DataType,
    pub var_name: String,
    pub init_expr: Option<Expr>,
}

#[derive(Debug)]
pub enum Stmt {
    Return {
        expr: Option<Expr>,
    },
    Assign {
        lval: String,
        expr: Expr,
    },
    Expression {
        expr: Option<Expr>,
    },
    Block {
        block: Block,
    },
    If {
        cond: Expr,
        then_body: Box<Stmt>, // Boxed to avoid recursive size issues
        else_body: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    Break,
    Continue,
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
    Call {
        func_name: String,
        args: Vec<Expr>,
    },
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

impl From<DataType> for FuncType {
    fn from(dt: DataType) -> Self {
        match dt {
            DataType::Int => FuncType::Int,
        }
    }
}