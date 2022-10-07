use std::collections::HashMap;

#[derive(Debug)]
struct Program {
    input: Type,
    output: Type,
    body: Node,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Type {
    Unit,
    Bool,
    Int,
}

#[derive(Clone, Debug)]
enum Value {
    Unit,
    Bool(bool),
    Int(i32),
}

impl Value {
    pub fn unwrap_bool(self) -> bool {
        if let Self::Bool(value) = self {
            value
        } else {
            panic!("expected bool, got: {:?}", self);
        }
    }

    pub fn unwrap_int(self) -> i32 {
        if let Self::Int(value) = self {
            value
        } else {
            panic!("expected int, got: {:?}", self);
        }
    }
}

#[derive(Debug)]
enum Node {
    /// let name = value
    Let {
        name: &'static str,
        value: Box<Self>,
    },

    /// name = value
    Assign {
        name: &'static str,
        value: Box<Self>,
    },

    /// E.g. 123
    Const(Value),

    /// E.g. foo
    Var(&'static str),

    /// lhs > rhs
    Gt { lhs: Box<Self>, rhs: Box<Self> },

    /// lhs + rhs
    Add { lhs: Box<Self>, rhs: Box<Self> },

    /// lhs - rhs
    Sub { lhs: Box<Self>, rhs: Box<Self> },

    /// while cond { body }
    While { cond: Box<Self>, body: Box<Self> },

    /// { ... }
    Block(Vec<Self>),
}

fn main() {
    let fib = Program {
        input: Type::Int,
        output: Type::Int,
        body: Node::Block(vec![
            // let x = 0
            Node::Let {
                name: "x",
                value: Box::new(Node::Const(Value::Int(0))),
            },
            // let y = 1
            Node::Let {
                name: "y",
                value: Box::new(Node::Const(Value::Int(1))),
            },
            // let z = 1
            Node::Let {
                name: "z",
                value: Box::new(Node::Const(Value::Int(1))),
            },
            // let n = input
            Node::Let {
                name: "n",
                value: Box::new(Node::Var("input")),
            },
            // while n > 0
            Node::While {
                cond: Box::new(Node::Gt {
                    lhs: Box::new(Node::Var("n")),
                    rhs: Box::new(Node::Const(Value::Int(0))),
                }),
                body: Box::new(Node::Block(vec![
                    // x = y
                    Node::Assign {
                        name: "x",
                        value: Box::new(Node::Var("y")),
                    },
                    // y = z
                    Node::Assign {
                        name: "y",
                        value: Box::new(Node::Var("z")),
                    },
                    // z = x + y
                    Node::Assign {
                        name: "z",
                        value: Box::new(Node::Add {
                            lhs: Box::new(Node::Var("x")),
                            rhs: Box::new(Node::Var("y")),
                        }),
                    },
                    // n = n - 1
                    Node::Assign {
                        name: "n",
                        value: Box::new(Node::Sub {
                            lhs: Box::new(Node::Var("n")),
                            rhs: Box::new(Node::Const(Value::Int(1))),
                        }),
                    },
                ])),
            },
            // x
            Node::Var("x"),
        ]),
    };

    let fib = compile::<i32, i32>(fib);

    println!("{}", fib(10));
}

trait IntoValue {
    fn into_value(self) -> Value;
    fn ty() -> Type;
}

impl IntoValue for bool {
    fn into_value(self) -> Value {
        Value::Bool(self)
    }

    fn ty() -> Type {
        Type::Bool
    }
}

impl IntoValue for i32 {
    fn into_value(self) -> Value {
        Value::Int(self)
    }

    fn ty() -> Type {
        Type::Int
    }
}

trait FromValue {
    fn from_value(value: Value) -> Self;
    fn ty() -> Type;
}

impl FromValue for bool {
    fn from_value(value: Value) -> Self {
        value.unwrap_bool()
    }

    fn ty() -> Type {
        Type::Bool
    }
}

impl FromValue for i32 {
    fn from_value(value: Value) -> Self {
        value.unwrap_int()
    }

    fn ty() -> Type {
        Type::Int
    }
}

fn compile<Input, Output>(prog: Program) -> impl Fn(Input) -> Output
where
    Input: IntoValue,
    Output: FromValue,
{
    let mut ctxt = CompilationContext {
        stack: vec![prog.input],
        vars: FromIterator::from_iter(vec![("input", 0)]),
    };

    let (ty, thunk) = compile_node(&mut ctxt, prog.body);

    assert_eq!(ty, prog.output);
    assert_eq!(Input::ty(), prog.input);
    assert_eq!(Output::ty(), prog.output);

    let stack_len = ctxt.stack.len();

    move |input: Input| -> Output {
        let mut ctxt = RuntimeContext {
            stack: vec![Value::Unit; stack_len],
        };

        ctxt.stack[0] = input.into_value();

        Output::from_value(thunk(&mut ctxt))
    }
}

type Thunk = Box<dyn Fn(&mut RuntimeContext) -> Value>;

struct CompilationContext {
    stack: Vec<Type>,
    vars: HashMap<&'static str, usize>,
}

struct RuntimeContext {
    stack: Vec<Value>,
}

fn compile_node(ctxt: &mut CompilationContext, node: Node) -> (Type, Thunk) {
    match node {
        Node::Let { name, value } => {
            let (ty, value) = compile_node(ctxt, *value);
            let id = ctxt.stack.len();

            ctxt.stack.push(ty);

            if ctxt.vars.insert(name, id).is_some() {
                panic!("var already declared: {}", name);
            }

            let ty = Type::Unit;

            let thunk = Box::new(move |ctxt: &mut RuntimeContext| {
                ctxt.stack[id] = value(ctxt);
                Value::Unit
            });

            (ty, thunk)
        }

        Node::Assign { name, value } => {
            let id = *ctxt.vars.get(name).unwrap_or_else(|| {
                panic!("var not defined: {}", name);
            });

            let (ty, value) = compile_node(ctxt, *value);

            assert_eq!(ty, ctxt.stack[id]);

            let ty = Type::Unit;

            let thunk = Box::new(move |ctxt: &mut RuntimeContext| {
                ctxt.stack[id] = value(ctxt);
                Value::Unit
            });

            (ty, thunk)
        }

        Node::Const(value) => {
            let ty = match &value {
                Value::Unit => Type::Unit,
                Value::Bool(_) => Type::Bool,
                Value::Int(_) => Type::Int,
            };

            let thunk = Box::new(move |_: &mut RuntimeContext| value.clone());

            (ty, thunk)
        }

        Node::Var(name) => {
            let id = *ctxt.vars.get(name).unwrap_or_else(|| {
                panic!("var not defined: {}", name);
            });

            let ty = ctxt.stack[id];

            let thunk = Box::new(move |ctxt: &mut RuntimeContext| ctxt.stack[id].clone());

            (ty, thunk)
        }

        Node::Gt { lhs, rhs } => {
            let (lhs_ty, lhs) = compile_node(ctxt, *lhs);
            let (rhs_ty, rhs) = compile_node(ctxt, *rhs);

            match (lhs_ty, rhs_ty) {
                (Type::Int, Type::Int) => {
                    let ty = Type::Bool;

                    let thunk = Box::new(move |ctxt: &mut RuntimeContext| {
                        let lhs = lhs(ctxt).unwrap_int();
                        let rhs = rhs(ctxt).unwrap_int();

                        Value::Bool(lhs > rhs)
                    });

                    (ty, thunk)
                }

                (lhs_ty, rhs_ty) => {
                    panic!("unknown op: {:?} > {:?}", lhs_ty, rhs_ty);
                }
            }
        }

        Node::Add { lhs, rhs } => {
            let (lhs_ty, lhs) = compile_node(ctxt, *lhs);
            let (rhs_ty, rhs) = compile_node(ctxt, *rhs);

            match (lhs_ty, rhs_ty) {
                (Type::Int, Type::Int) => {
                    let ty = Type::Int;

                    let thunk = Box::new(move |ctxt: &mut RuntimeContext| {
                        let lhs = lhs(ctxt).unwrap_int();
                        let rhs = rhs(ctxt).unwrap_int();

                        Value::Int(lhs + rhs)
                    });

                    (ty, thunk)
                }

                (lhs_ty, rhs_ty) => {
                    panic!("unknown op: {:?} + {:?}", lhs_ty, rhs_ty);
                }
            }
        }

        Node::Sub { lhs, rhs } => {
            let (lhs_ty, lhs) = compile_node(ctxt, *lhs);
            let (rhs_ty, rhs) = compile_node(ctxt, *rhs);

            match (lhs_ty, rhs_ty) {
                (Type::Int, Type::Int) => {
                    let ty = Type::Int;

                    let thunk = Box::new(move |ctxt: &mut RuntimeContext| {
                        let lhs = lhs(ctxt).unwrap_int();
                        let rhs = rhs(ctxt).unwrap_int();

                        Value::Int(lhs - rhs)
                    });

                    (ty, thunk)
                }

                (lhs_ty, rhs_ty) => {
                    panic!("unknown op: {:?} - {:?}", lhs_ty, rhs_ty);
                }
            }
        }

        Node::While { cond, body } => {
            let (cond_ty, cond) = compile_node(ctxt, *cond);
            let (_, body) = compile_node(ctxt, *body);

            assert_eq!(Type::Bool, cond_ty);

            let ty = Type::Unit;

            let thunk = Box::new(move |ctxt: &mut RuntimeContext| {
                while cond(ctxt).unwrap_bool() {
                    body(ctxt);
                }

                Value::Unit
            });

            (ty, thunk)
        }

        Node::Block(nodes) => {
            let (tys, nodes): (Vec<_>, Vec<_>) = nodes
                .into_iter()
                .map(|node| compile_node(ctxt, node))
                .unzip();

            let ty = tys.into_iter().last().unwrap();

            let thunk = Box::new(move |ctxt: &mut RuntimeContext| {
                let mut value = Value::Unit;

                for node in &nodes {
                    value = node(ctxt);
                }

                value
            });

            (ty, thunk)
        }
    }
}
