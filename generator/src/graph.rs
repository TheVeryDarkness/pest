// pest. The Elegant Parser
// Copyright (c) 2018 Dragoș Tiselice
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crate::types::{option_type, vec_type};
use pest_meta::{ast::RuleType, optimizer::OptimizedRule};
use proc_macro2::{Ident, TokenStream};
pub use std::collections::BTreeMap as Map;

use crate::optimizer::OptimizedExpr;

fn ident(s: &str) -> Ident {
    format_ident!("r#{}", s)
}
fn quote_ident(s: &str) -> TokenStream {
    let ident = ident(s);
    quote! {#ident}
}

fn fn_decl() -> TokenStream {
    quote! {
        #[inline]
        fn try_new(input: &'i str, line_index: ::pest::iterators::Option<::pest::iterators::Rc<::pest::iterators::line_index::LineIndex>>) -> Result<(&'i ::std::primitive::str, Self), ::pest::error::Error<R>>
    }
}

/// Returns type name
fn generate_graph_node(
    expr: &OptimizedExpr,
    candidate_name: String,
    // From node name to type definition and implementation
    map: &mut Map<String, TokenStream>,
) -> TokenStream {
    macro_rules! walk_tree {
        ($ivar: ident, $ovar: ident) => {{
            let mut nodes: Vec<&OptimizedExpr> = Vec::new();
            let mut names: Vec<TokenStream> = Vec::new();
            let mut i = 0usize;
            let mut current = expr;
            while let OptimizedExpr::$ivar(lhs, rhs) = current {
                nodes.push(lhs);
                names.push(generate_graph_node(
                    lhs,
                    format!("{}_{}", candidate_name, i),
                    map,
                ));
                current = rhs;
                i += 1;
            }
            nodes.push(current);
            (nodes, names, quote_ident(&candidate_name))
        }};
    }

    let f = fn_decl();
    let vec = vec_type();
    let option = option_type();
    let s = quote!(&'i ::std::primitive::str);
    let attr = quote! {}; // quote! {#[allow(non_snake_case)]};
    let mut copy_if_forced =
        |candidate_name: String, type_name: TokenStream, fimpl: TokenStream| -> TokenStream {
            let name = ident(&candidate_name);
            let inner = type_name.clone();
            let s = s.clone();
            let def = quote! {
                #attr
                pub struct #name<'i> {
                    pub span: #s,
                    pub content: #inner,
                }
                impl<'i, R: ::pest::RuleType> ::pest::iterators::TypedNode<'i, R> for #name<'i> {
                    #f {
                        #fimpl
                    }
                }
            };
            map.insert(candidate_name.clone(), def);
            quote_ident(&candidate_name)
        };
    // Still some compile-time information not taken
    match expr {
        OptimizedExpr::Str(content) => copy_if_forced(
            candidate_name,
            s.clone(),
            quote! {
                if input.starts_with(#content) {
                    let (matched, remained) = input.split_at(#content.len());
                    return Ok((remained, Self{ span: matched, content: matched }));
                }
                todo!();
            },
        ),
        OptimizedExpr::Insens(_) => copy_if_forced(candidate_name, s.clone(), quote! {todo!()}),
        OptimizedExpr::PeekSlice(_, _) | OptimizedExpr::Push(_) | OptimizedExpr::Skip(_) => {
            copy_if_forced(candidate_name, s.clone(), quote!(todo!()))
        }
        OptimizedExpr::Range(start, end) => {
            let start = start.chars().next().unwrap();
            let end = end.chars().next().unwrap();
            copy_if_forced(
                candidate_name,
                quote! {::std::primitive::char},
                quote! {
                    if let Some(first) = input.chars().next() {
                        if #start <= first && first <= #end {
                            let (matched, remained) = input.split_at(first.len());
                            return Ok((remained, Self { span: matched, content: matched }));
                        }
                    }
                    todo!();
                },
            )
        }
        OptimizedExpr::Ident(id) => {
            let name = quote_ident(id);
            copy_if_forced(candidate_name, quote! {#name::<'i>}, quote!(todo!()))
        }
        OptimizedExpr::PosPred(_) | OptimizedExpr::NegPred(_) | OptimizedExpr::RestoreOnErr(_) => {
            copy_if_forced(candidate_name, quote! {()}, quote!(todo!()))
        }
        OptimizedExpr::Seq(_lhs, _rhs) => {
            let (_nodes, names, res) = walk_tree!(Seq, Sequence);
            let name = ident(&candidate_name);
            let (init, fields): (Vec<_>, Vec<_>) = names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let field = format_ident!("r#field{}", i);
                    (
                        quote! { let (input, #field) = #name::<'i>::try_new(input, line_index)?;  },
                        field,
                    )
                })
                .unzip();
            let def = quote! {
                #attr
                pub struct #name<'i>{
                    // span: &'i [char],
                    #(pub #fields: #names::<'i>),*
                }
                impl<'i, R: ::pest::RuleType> ::pest::iterators::TypedNode<'i, R> for #name<'i> {
                    #f {
                        #(#init)*
                        Ok( Self { #(#fields),* } )
                    }
                }
            };
            map.entry(candidate_name.clone()).or_insert(def);

            res
        }
        OptimizedExpr::Choice(_lhs, _rhs) => {
            let (_nodes, names, res) = walk_tree!(Choice, Variant);
            let name = ident(&candidate_name);
            let vars = names
                .iter()
                .enumerate()
                .map(|(i, _n)| format_ident!("var_{}", i));
            let init = names.iter().enumerate().map(|(i, var)| {
                let var_name = format_ident!("var_{}", i);
                quote! {
                    if let Ok((input, res)) = #var::<'i>::try_new(input, line_index) {
                        return Ok((input, #name::#var_name(res)));
                    }
                }
            });
            let def = quote! {
                #attr
                pub enum #name<'i> {
                    #( #vars(#names::<'i>) ),*
                }
                impl<'i, R: ::pest::RuleType> ::pest::iterators::TypedNode<'i, R> for #name<'i> {
                    #f {
                        #(#init)*
                        panic!("All branches failed.");
                    }
                }
            };
            map.entry(candidate_name.clone()).or_insert(def);
            res
        }
        OptimizedExpr::Opt(inner) => {
            let name = ident(&candidate_name);
            let inner_name = generate_graph_node(inner, format!("{}_o", candidate_name), map);
            let def = quote! {
                #attr
                pub struct #name<'i>(#option::<#inner_name::<'i>>);
                impl<'i, R: ::pest::RuleType> ::pest::iterators::TypedNode<'i, R> for #name<'i> {
                    #f {
                        match #inner_name::<'i>::try_new(input, line_index) {
                            Ok((input, inner)) => Ok((input, Some(inner))),
                            Err(_) => Ok((input, None))
                        }
                    }
                }
            };
            map.entry(candidate_name.clone()).or_insert(def);
            quote_ident(&candidate_name)
        }
        OptimizedExpr::Rep(inner) => {
            let name = ident(&candidate_name);
            let inner_name = generate_graph_node(inner, format!("{}_r", candidate_name), map);
            let def = quote! {
                #attr
                pub struct #name<'i>(#vec::<#inner_name::<'i>>);
                impl<'i, R: ::pest::RuleType> ::pest::iterators::TypedNode<'i, R> for #name<'i> {
                    #f {
                        let vec = #vec::<#inner_name::<'i>>::new();
                        let mut input = input;
                        while let Ok(next, elem) = #inner_name::<'i>::try_new(input, line_index) {
                            input = next;
                            vec.push(elem);
                        }
                        Ok((input, vec))
                    }
                }
            };
            map.entry(candidate_name.clone()).or_insert(def);
            quote_ident(&candidate_name)
        }
        #[cfg(feature = "grammar-extras")]
        OptimizedExpr::NodeTag(inner_expr, _tag) => {
            generate_graph_node(inner_expr, format!("{}_0", candidate_name), map)
        }
    }
}

pub fn generate_graph(rules: &[OptimizedRule]) -> Map<String, TokenStream> {
    let mut res = Map::<String, TokenStream>::new();
    let f = fn_decl();
    for rule in rules.iter() {
        match rule.ty {
            RuleType::Normal
            | RuleType::Silent
            | RuleType::NonAtomic
            | RuleType::CompoundAtomic => {
                generate_graph_node(&rule.expr, rule.name.clone(), &mut res);
            }
            RuleType::Atomic => {
                let name = ident(&rule.name);
                let def = quote!(
                    struct #name<'i>(&'i ::std::primitive::str);

                    impl<'i, R: ::pest::RuleType> ::pest::iterators::TypedNode<'i, R> for #name<'i> {
                        #f {
                            todo!()
                        }
                    }
                );
                res.entry(rule.name.clone()).or_insert(def);
            }
        }
    }
    res
}

pub fn generate_typed_pair_from_rule(rules: &[OptimizedRule]) -> TokenStream {
    let graph = generate_graph(rules);
    let pairs = graph.iter().map(|(_name, rule)| rule);
    // let names = rules.iter().map(|rule| format_ident!("r#{}", rule.name));
    let res = quote! {
        pub mod pairs {
            pub type ANY<'_i> = char;
            pub type SOI<'_i> = ();
            pub type EOI<'_i> = ();
            pub type NEWLINE<'_i> = ();
            #( #pairs )*
        }
    };
    println!("{}", res);
    res
}
