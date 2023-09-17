use std::collections::{BTreeMap, HashMap};

use super::expression::eval_expression;
use crate::ast::Statement;
use crate::runtime::std::Prototypes;
use crate::runtime::value::{KeyValue, Value};
use crate::runtime::{DeclType, ScopeStack};
use crate::Export;

#[derive(Debug, Clone)]
pub enum Escape {
    None,
    Return(Value),
    Break,
    Continue,
}

pub fn eval_statement(
    scopes: &mut ScopeStack,
    statement: Statement,
    modules: Vec<Export>,
    prototypes: Prototypes,
) -> Result<Escape, String> {
    match statement {
        Statement::ExpressionStatement(expr) => {
            eval_expression(scopes, expr, modules, prototypes)?;
        }
        Statement::LetStatement(name, rhs) => {
            let value = eval_expression(scopes, rhs, modules, prototypes)?;
            scopes.declare(name, value, DeclType::Mutable)?;
        }
        Statement::ConstStatement(name, rhs) => {
            let value = eval_expression(scopes, rhs, modules, prototypes)?;
            scopes.declare(name, value, DeclType::Immutable)?;
        }
        Statement::ImportStatement(args, items) => {
            apply_imports(scopes, modules, args, items)?;
        }
        Statement::AssignmentStatement(name, rhs) => {
            let value = eval_expression(scopes, rhs, modules, prototypes)?;
            scopes.assgin(name, value)?;
        }
        Statement::IfStatement(branchs, else_block) => {
            for branch in branchs {
                let value = eval_expression(
                    scopes,
                    branch.condition,
                    modules.clone(),
                    prototypes.clone(),
                )?;

                match value {
                    Value::Bool(b) => {
                        if b {
                            let ret = eval_statements(
                                scopes,
                                branch.statements,
                                modules.clone(),
                                prototypes.clone(),
                            )?;
                            return Ok(ret);
                        }
                    }
                    _ => return Err(format!("condition most be a boolean")),
                }
            }

            if let Some(stmts) = else_block {
                let e = eval_statements(scopes, stmts, modules.clone(), prototypes.clone())?;
                return Ok(e);
            }
        }
        Statement::ReturnStatement(expr) => {
            let value = eval_expression(scopes, expr, modules.clone(), prototypes.clone())?;
            return Ok(Escape::Return(value));
        }
        Statement::FnStatement(name, args, block) => {
            scopes.declare(name, Value::Func(args, block), DeclType::Immutable)?;
        }
        Statement::ForStatement(lhs, iter, block) => {
            let iter_val = eval_expression(scopes, iter, modules.clone(), prototypes.clone())?;

            match iter_val {
                Value::List(values) => {
                    for value in values {
                        let mut inner_scopes = scopes.new_from_push(HashMap::new());

                        inner_scopes.declare(lhs.clone(), value, DeclType::Mutable)?;
                        let ret = eval_statements(
                            &mut inner_scopes,
                            block.to_vec(),
                            modules.clone(),
                            prototypes.clone(),
                        )?;

                        match ret {
                            Escape::None => {}
                            Escape::Continue => {}
                            Escape::Return(v) => return Ok(Escape::Return(v)),
                            Escape::Break => return Ok(Escape::None),
                        }
                    }
                }
                _ => return Err(format!("iterator most be a list")),
            }
        }
        Statement::BreakStatement => return Ok(Escape::Break),
        Statement::ContinueStatement => return Ok(Escape::Continue),
        Statement::WhileStatement(cond, block) => loop {
            let value = eval_expression(scopes, cond.clone(), modules.clone(), prototypes.clone())?;

            match value {
                Value::Bool(b) => {
                    if !b {
                        break;
                    }

                    let ret = eval_statements(
                        scopes,
                        block.clone(),
                        modules.clone(),
                        prototypes.clone(),
                    )?;

                    match ret {
                        Escape::None => {}
                        Escape::Continue => {}
                        Escape::Return(v) => return Ok(Escape::Return(v)),
                        Escape::Break => return Ok(Escape::None),
                    }
                }
                _ => return Err(format!("condition most be a boolean")),
            }
        },
        Statement::ModuleStatement(name, statements) => {
            let module = eval_module(scopes, modules, prototypes, name.to_string(), statements)?;

            scopes.declare(name, Value::Module(module), DeclType::Immutable)?;
        }
    };

    Ok(Escape::None)
}

pub fn eval_statements(
    scopes: &mut ScopeStack,
    statements: Vec<Statement>,
    modules: Vec<Export>,
    prototypes: Prototypes,
) -> Result<Escape, String> {
    let mut inner_scopes = scopes.new_from_push(HashMap::new());

    for statement in &statements {
        let e = eval_statement(
            &mut inner_scopes,
            statement.clone(),
            modules.clone(),
            prototypes.clone(),
        )?;

        if let Statement::FnStatement(_, _, _) = statement {
            continue;
        }

        if let Escape::None = e {
            continue;
        }

        return Ok(e);
    }

    Ok(Escape::None)
}

pub fn eval_module(
    scopes: &mut ScopeStack,
    modules: Vec<Export>,
    prototypes: Prototypes,
    name: String,
    statements: Vec<Statement>,
) -> Result<BTreeMap<String, Value>, String> {
    let mut exports: BTreeMap<String, Value> = BTreeMap::new();

    let mut inner_scope = scopes.new_from_push(HashMap::new());
    for statement in statements {
        match statement {
            Statement::ConstStatement(name, expr) => {
                let value =
                    eval_expression(&mut inner_scope, expr, modules.clone(), prototypes.clone())?;

                exports.insert(name, value);
            }
            Statement::LetStatement(name, expr) => {
                let value =
                    eval_expression(&mut inner_scope, expr, modules.clone(), prototypes.clone())?;

                exports.insert(name, value);
            }
            Statement::FnStatement(name, args, block) => {
                exports.insert(name, Value::Func(args, block));
            }
            Statement::ModuleStatement(name2, statements2) => {
                let exports2 = eval_module(
                    &mut inner_scope,
                    modules.clone(),
                    prototypes.clone(),
                    name2.to_string(),
                    statements2,
                )?;
                exports.insert(name2, Value::Module(exports2));
            }
            other => return Err(format!("'{:?}' is not supported in modules", other)),
        }
    }

    inner_scope.declare(name, Value::Module(exports.clone()), DeclType::Immutable)?;
    Ok(exports)
}

pub fn apply_imports(
    scopes: &mut ScopeStack,
    modules: Vec<Export>,
    args: Vec<String>,
    items: Option<Vec<String>>,
) -> Result<(), String> {
    let mut last = modules;

    for (i, arg) in args.iter().enumerate() {
        if let Some(m) = last.to_vec().into_iter().find(|e| match e {
            Export::Module { name, exports: _ } => {
                return name == arg;
            }
            Export::Item { name, value: _ } => {
                return name == arg;
            }
        }) {
            match m {
                Export::Module { name: _, exports } => {
                    if let None = args.get(i + 1) {
                        if let Some(items) = &items {
                            for export in exports.iter() {
                                match export {
                                    Export::Module { name: n1, exports } => {
                                        let mut obj: Vec<KeyValue> = vec![];
                                        for export in exports.iter() {
                                            if let Export::Item { name: n, value } = export {
                                                obj.push(KeyValue {
                                                    key: n.to_string(),
                                                    value: value.clone(),
                                                });
                                            }
                                        }
                                        scopes.declare_builtin(
                                            n1.to_string(),
                                            Value::Object(obj),
                                            DeclType::Immutable,
                                        )?;
                                    }
                                    Export::Item { name, value } => {
                                        if items.contains(&name) {
                                            scopes.declare_builtin(
                                                name.to_string(),
                                                value.clone(),
                                                DeclType::Immutable,
                                            )?;
                                        }
                                    }
                                }
                            }
                        } else {
                            let mut obj: Vec<KeyValue> = vec![];
                            for export in exports.iter() {
                                if let Export::Item { name, value } = export {
                                    obj.push(KeyValue {
                                        key: name.to_string(),
                                        value: value.clone(),
                                    });
                                }
                            }
                            scopes.declare_builtin(
                                arg.to_string(),
                                Value::Object(obj),
                                DeclType::Immutable,
                            )?;
                        }
                    } else {
                        last = exports.to_owned();
                    }
                }
                Export::Item { name, value } => {
                    if let Some(_) = items {
                        return Err(format!("{} is not a module", name));
                    }
                    if let Some(_) = args.get(i + 1) {
                        return Err(format!("{} is not a module", arg));
                    } else {
                        scopes.declare_builtin(
                            arg.to_string(),
                            value.to_owned(),
                            DeclType::Immutable,
                        )?;
                    }
                }
            }
        } else {
            return Err(format!("module or item {} not found", arg));
        }
    }

    Ok(())
}
