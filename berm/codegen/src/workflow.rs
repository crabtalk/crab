use crate::{record, schema};
use berm_abi::{ABI_SECTION, ABI_VERSION, WORKFLOW_PREFIX};
use heck::ToSnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::BTreeSet;
use syn::{
    FnArg, GenericArgument, ImplItem, ImplItemFn, ItemImpl, Pat, PathArguments, ReturnType, Type,
    ext::IdentExt,
};

struct Workflow {
    item: ItemImpl,
    name: String,
    nodes: Vec<Node>,
    input: Type,
    output: Type,
}
struct Node {
    method: ImplItemFn,
    name: String,
    kind: Kind,
    result: Type,
    fallible: bool,
    parameters: Vec<Parameter>,
}
enum Kind {
    Once(Type),
    Optional(Type),
    Each { call: Type, max: u32 },
    Branch,
    Output,
}
struct Parameter {
    name: String,
    owned: Type,
    optional: bool,
    borrowed: Type,
}

pub fn expand(item: ItemImpl) -> syn::Result<TokenStream> {
    let workflow = Workflow::read(item)?;
    let record = workflow.record()?;
    let exports = workflow.exports();
    let item = &workflow.item;
    let ty = &item.self_ty;
    Ok(quote! { #item const _: #ty = #ty; #record #exports })
}

impl Workflow {
    fn read(mut item: ItemImpl) -> syn::Result<Self> {
        if item.trait_.is_some()
            || !item.generics.params.is_empty()
            || item.generics.where_clause.is_some()
            || item.unsafety.is_some()
        {
            return Err(syn::Error::new_spanned(
                &item,
                "#[workflow] requires an inherent impl without generics on a unit struct",
            ));
        }
        let Type::Path(path) = &*item.self_ty else {
            return Err(syn::Error::new_spanned(
                &item.self_ty,
                "a workflow is a unit struct",
            ));
        };
        let name = path
            .path
            .segments
            .last()
            .unwrap()
            .ident
            .unraw()
            .to_string()
            .to_snake_case();
        let mut nodes = Vec::new();
        let mut names = BTreeSet::new();
        let mut input = None;
        let mut output = None;
        for member in &mut item.items {
            let ImplItem::Fn(method) = member else {
                return Err(syn::Error::new_spanned(
                    member,
                    "a workflow impl contains only step, branch and output methods",
                ));
            };
            let node = Node::read(method)?;
            if node.name == "input" || names.contains(&node.name) {
                return Err(syn::Error::new_spanned(
                    &method.sig.ident,
                    "workflow node names are unique and input is reserved",
                ));
            }
            for parameter in &node.parameters {
                if parameter.name == "input" {
                    input.get_or_insert_with(|| parameter.owned.clone());
                } else if !names.contains(&parameter.name) {
                    return Err(syn::Error::new_spanned(
                        &method.sig.inputs,
                        format!(
                            "{} depends on unknown or later node {}; use an earlier method name",
                            node.name, parameter.name
                        ),
                    ));
                }
            }
            if matches!(node.kind, Kind::Output) {
                if output.is_some() {
                    return Err(syn::Error::new_spanned(
                        &method.sig.ident,
                        "a workflow has exactly one #[output]",
                    ));
                }
                output = Some(node.result.clone());
            }
            names.insert(node.name.clone());
            nodes.push(node);
        }
        let input = input.ok_or_else(|| {
            syn::Error::new_spanned(&item, "a workflow needs at least one input dependency")
        })?;
        let output = output.ok_or_else(|| {
            syn::Error::new_spanned(&item, "a workflow needs exactly one #[output]")
        })?;
        Ok(Self {
            item,
            name,
            nodes,
            input,
            output,
        })
    }

    fn producer(&self, parameter: &Parameter) -> Option<(usize, &Node)> {
        self.nodes
            .iter()
            .enumerate()
            .find(|(_, node)| node.name == parameter.name)
    }

    fn record(&self) -> syn::Result<TokenStream> {
        let krate = std::env::var("CARGO_PKG_NAME")
            .map_err(|_| syn::Error::new_spanned(&self.item, "#[workflow] needs CARGO_PKG_NAME"))?;
        let ids: Vec<_> = self.nodes.iter().map(|node| &node.name).collect();
        let n = ids.len();
        let capacity = n * n;
        let static_nodes = self.nodes.iter().map(|node| {
            let kind = match node.kind {
                Kind::Once(_) => quote!(::berm_lang::abi::workflow::Kind::Step { maximum: 1, optional: false }),
                Kind::Optional(_) => quote!(::berm_lang::abi::workflow::Kind::Step { maximum: 1, optional: true }),
                Kind::Each { max, .. } => quote!(::berm_lang::abi::workflow::Kind::Step { maximum: #max, optional: false }),
                Kind::Branch => quote!(::berm_lang::abi::workflow::Kind::Branch),
                Kind::Output => quote!(::berm_lang::abi::workflow::Kind::Output),
            };
            let dependencies = node.parameters.iter().map(|parameter| {
                let optional = parameter.optional;
                let (producer, case) = match self.producer(parameter) {
                    Some((index, producer)) => {
                        let output = producer.value_type();
                        let ty = &parameter.owned;
                        (quote!(Some(#index)), quote!(<#output as ::berm_lang::workflow::Projection<#ty>>::CASE))
                    }
                    None => (quote!(None), quote!(None)),
                };
                quote!(::berm_lang::abi::workflow::Dependency { producer: #producer, optional: #optional, case: #case })
            });
            quote!(::berm_lang::abi::workflow::Node { kind: #kind, dependencies: &[#(#dependencies),*] })
        });
        let errors = self.nodes.iter().enumerate().map(|(index, node)| {
            let join = format!("{}: required dependencies select mutually exclusive paths; use optional references", node.name);
            let output = format!("{}: output must be reachable on every path; use optional references for guarded values", node.name);
            let disconnected = format!("{}: node does not contribute to the workflow output", node.name);
            quote! {
                (#index, ::berm_lang::abi::workflow::InvalidKind::Join) => panic!(#join),
                (#index, ::berm_lang::abi::workflow::InvalidKind::Output) => panic!(#output),
                (#index, ::berm_lang::abi::workflow::InvalidKind::Disconnected) => panic!(#disconnected),
            }
        });
        let mut out = record::Pieces::default();
        out.text(&format!(
            r#"{{"workflow":{{"version":1,"name":{},"graph":{{"input":"#,
            serde_json::Value::from(self.name.as_str())
        ));
        let input = &self.input;
        out.shape(quote!(<#input as ::berm_lang::Schema>::SHAPE));
        out.text(r#","output":"#);
        let output = &self.output;
        out.shape(quote!(<#output as ::berm_lang::Schema>::SHAPE));
        out.text(r#","nodes":["#);
        for (index, node) in self.nodes.iter().enumerate() {
            if index > 0 {
                out.text(",");
            }
            let symbol = format!("{WORKFLOW_PREFIX}{}_{}", self.name, node.name);
            out.text(&format!(
                r#"{{"id":{},"export":{},"dependencies":["#,
                serde_json::Value::from(node.name.as_str()),
                serde_json::Value::from(symbol)
            ));
            for (at, parameter) in node.parameters.iter().enumerate() {
                if at > 0 {
                    out.text(",");
                }
                match self.producer(parameter) {
                    Some((_, producer)) => {
                        out.text(&format!(
                            r#"{{"source":{{"kind":"node","id":{}}},"optional":{},"case":"#,
                            serde_json::Value::from(producer.name.as_str()),
                            parameter.optional
                        ));
                        let output = producer.value_type();
                        let ty = &parameter.owned;
                        out.piece(quote!(::berm_lang::schema::Piece::MaybeString(<#output as ::berm_lang::workflow::Projection<#ty>>::CASE)));
                    }
                    None => out.text(&format!(
                        r#"{{"source":{{"kind":"input"}},"optional":{},"case":null"#,
                        parameter.optional
                    )),
                }
                out.text("}");
            }
            out.text(r#"],"guards":"#);
            out.piece(quote!(::berm_lang::schema::Piece::Guards { ids: &[#(#ids),*], selections: &GUARDS, node: #index }));
            match &node.kind {
                Kind::Once(call) | Kind::Optional(call) | Kind::Each { call, .. } => {
                    out.text(r#","kind":"step","call":{"target":"#);
                    out.piece(quote!(::berm_lang::schema::Piece::Target(&<#call as ::berm_lang::workflow::Call>::TARGET)));
                    out.text(r#","input":"#);
                    out.shape(quote!(<#call as ::berm_lang::Schema>::SHAPE));
                    out.text(r#","output":"#);
                    out.shape(quote!(<<#call as ::berm_lang::workflow::Call>::Output as ::berm_lang::Schema>::SHAPE));
                    out.text(r#","cases":"#);
                    out.piece(quote!(::berm_lang::schema::Piece::Cases(<#call as ::berm_lang::workflow::Call>::CASES)));
                    out.text(r#"},"mode":"#);
                    out.text(&match node.kind {
                        Kind::Once(_) => r#"{"kind":"once"}"#.into(),
                        Kind::Optional(_) => r#"{"kind":"optional"}"#.into(),
                        Kind::Each { max, .. } => format!(r#"{{"kind":"each","max":{max}}}"#),
                        _ => unreachable!(),
                    });
                }
                Kind::Branch => {
                    out.text(r#","kind":"branch","cases":"#);
                    let ty = &node.result;
                    out.piece(quote!(::berm_lang::schema::Piece::Cases(<#ty as ::berm_lang::workflow::Branch>::CASES)));
                }
                Kind::Output => out.text(r#","kind":"output""#),
            }
            out.text("}");
        }
        out.text(&format!(
            r#"]}}}},"abi_version":{ABI_VERSION},"crate":{}}}"#,
            serde_json::Value::from(krate)
        ));
        let record = out.array();
        let ty = &self.item.self_ty;
        Ok(quote! {
            impl #ty {
                pub const WORKFLOW: &'static [u8] = &{
                const NODES: &[::berm_lang::abi::workflow::Node<'static>] = &[#(#static_nodes),*];
                const GUARDS: [Option<::berm_lang::abi::workflow::Selection<'static>>; #capacity] = {
                    let mut guards = [None; #capacity];
                    let mut reachable = [false; #n];
                    if let Err(error) = ::berm_lang::abi::workflow::validate(NODES, &mut guards, &mut reachable, 100) {
                        match (error.node, error.kind) {
                            #(#errors)*
                            _ => panic!("invalid workflow graph"),
                        }
                    }
                    guards
                };
                #record
                };
            }
            const _: () = {
                #[used]
                #[cfg_attr(target_arch = "wasm32", unsafe(link_section = #ABI_SECTION))]
                static RECORD: [u8; <#ty>::WORKFLOW.len()] = {
                    let mut bytes = [0; <#ty>::WORKFLOW.len()];
                    let mut at = 0;
                    while at < bytes.len() {
                        bytes[at] = <#ty>::WORKFLOW[at];
                        at += 1;
                    }
                    bytes
                };
            };
        })
    }

    fn exports(&self) -> TokenStream {
        let ty = &self.item.self_ty;
        let input = &self.input;
        let exports = self.nodes.iter().map(|node| {
            let symbol = format_ident!("{WORKFLOW_PREFIX}{}_{}", self.name, node.name);
            let method = &node.method.sig.ident;
            let fallible = node.fallible.then(|| quote!(?));
            let mut decode = Vec::new();
            let arguments = node.parameters.iter().map(|parameter| {
                let owned = &parameter.owned;
                let borrowed = &parameter.borrowed;
                let borrow = quote!(<#owned as ::core::borrow::Borrow<#borrowed>>::borrow);
                if parameter.name == "input" {
                    let value = quote!(#borrow(&__berm_input));
                    return if parameter.optional { quote!(Some(#value)) } else { value };
                }
                let id = &parameter.name;
                let variable = format_ident!("__berm_{}", id);
                let (_, producer) = self.producer(parameter).unwrap();
                let output = producer.value_type();
                let missing = format!("missing workflow dependency {id}");
                decode.push(quote! {
                    let #variable: Option<#owned> = match __berm_args.nodes.get(#id) {
                        Some(::berm_lang::abi::workflow::ControlValue::Present { value }) => {
                            let producer: #output = ::berm_lang::serde_json::from_value(value.clone())?;
                            <#output as ::berm_lang::workflow::Projection<#owned>>::project(producer)
                        }
                        Some(::berm_lang::abi::workflow::ControlValue::Skipped) => None,
                        None => return Err(#missing.into()),
                    };
                });
                let value = quote!(#variable.as_ref().map(#borrow));
                if parameter.optional {
                    value
                } else {
                    let skipped = format!("required workflow dependency {id} is skipped");
                    quote!(#value.ok_or(#skipped)?)
                }
            }).collect::<Vec<_>>();
            let prepare = match &node.kind {
                Kind::Once(_) => quote!(::berm_lang::abi::workflow::ControlOutput::Calls { inputs: ::berm_lang::__private::Vec::from([::berm_lang::serde_json::to_value(result)?]) }),
                Kind::Optional(_) => quote!(match result {
                    Some(call) => ::berm_lang::abi::workflow::ControlOutput::Calls { inputs: ::berm_lang::__private::Vec::from([::berm_lang::serde_json::to_value(call)?]) },
                    None => ::berm_lang::abi::workflow::ControlOutput::Skipped,
                }),
                Kind::Each { max, .. } => {
                    let message = format!("{} exceeds its each bound of {max}", node.name);
                    quote!({
                        if result.len() > #max as usize { return Err(#message.into()); }
                        let mut inputs = ::berm_lang::__private::Vec::with_capacity(result.len());
                        for call in result { inputs.push(::berm_lang::serde_json::to_value(call)?); }
                        ::berm_lang::abi::workflow::ControlOutput::Calls { inputs }
                    })
                }
                Kind::Branch => quote!(::berm_lang::abi::workflow::ControlOutput::Branch {
                    case: ::berm_lang::workflow::Branch::case(&result).into(),
                    value: ::berm_lang::serde_json::to_value(result)?,
                }),
                Kind::Output => quote!(::berm_lang::abi::workflow::ControlOutput::Output { value: ::berm_lang::serde_json::to_value(result)? }),
            };
            quote! {
                const _: extern "C" fn() = #symbol;
                #[cfg_attr(target_arch = "wasm32", unsafe(no_mangle))]
                pub extern "C" fn #symbol() {
                    ::berm_lang::dispatch::run(|bytes| {
                        let __berm_args: ::berm_lang::abi::workflow::ControlInput = ::berm_lang::dispatch::args(bytes)?;
                        let __berm_input: #input = ::berm_lang::serde_json::from_value(__berm_args.input)?;
                        #(#decode)*
                        let result = <#ty>::#method(#(#arguments),*) #fallible;
                        ::berm_lang::dispatch::reply(&#prepare)
                    });
                }
            }
        });
        quote!(#(#exports)*)
    }
}

impl Node {
    fn read(method: &mut ImplItemFn) -> syn::Result<Self> {
        let signature = &method.sig;
        if signature.asyncness.is_some()
            || signature.constness.is_some()
            || signature.unsafety.is_some()
            || signature.abi.is_some()
            || signature.variadic.is_some()
            || !signature.generics.params.is_empty()
            || signature.generics.where_clause.is_some()
        {
            return Err(syn::Error::new_spanned(
                signature,
                "workflow methods are plain functions without self, async, generics or impl Trait",
            ));
        }
        let attributes: Vec<_> = method
            .attrs
            .iter()
            .filter(|attribute| {
                ["step", "branch", "output"]
                    .iter()
                    .any(|name| attribute.path().is_ident(name))
            })
            .cloned()
            .collect();
        if attributes.len() != 1 {
            return Err(syn::Error::new_spanned(
                signature,
                "each workflow method has exactly one of #[step], #[branch] and #[output]",
            ));
        }
        let attribute = &attributes[0];
        let declared = match &signature.output {
            ReturnType::Type(_, declared) => (**declared).clone(),
            ReturnType::Default if attribute.path().is_ident("output") => syn::parse_quote!(()),
            ReturnType::Default => {
                return Err(syn::Error::new_spanned(
                    signature,
                    "workflow methods need an explicit return type",
                ));
            }
        };
        let declared = &declared;
        let fallible = schema::inner(declared, "Result").is_some();
        let result = schema::inner(declared, "Result")
            .unwrap_or(declared)
            .clone();
        concrete(&result)?;
        let kind = if attribute.path().is_ident("step") {
            let mut each = false;
            let mut maximum = None;
            if !matches!(attribute.meta, syn::Meta::Path(_)) {
                attribute.parse_nested_meta(|meta| {
                    if meta.path.is_ident("each") && !each {
                        each = true;
                        Ok(())
                    } else if meta.path.is_ident("max") && maximum.is_none() {
                        maximum = Some(
                            meta.value()?
                                .parse::<syn::LitInt>()?
                                .base10_parse::<u32>()?,
                        );
                        Ok(())
                    } else {
                        Err(meta.error("expected each and one max = positive integer"))
                    }
                })?;
            }
            match (each, maximum) {
                (true, Some(max @ 1..=100)) => {
                    let call = schema::inner(&result, "Vec")
                        .ok_or_else(|| {
                            syn::Error::new_spanned(declared, "an each step returns Vec<Call>")
                        })?
                        .clone();
                    Kind::Each { call, max }
                }
                (true, _) => {
                    return Err(syn::Error::new_spanned(
                        attribute,
                        "each requires max = N, where N is a positive integer no larger than 100",
                    ));
                }
                (false, Some(_)) => {
                    return Err(syn::Error::new_spanned(attribute, "max requires each"));
                }
                (false, None) => match schema::inner(&result, "Option") {
                    Some(call) => Kind::Optional(call.clone()),
                    None => Kind::Once(result.clone()),
                },
            }
        } else {
            if !matches!(attribute.meta, syn::Meta::Path(_)) {
                attribute.parse_args::<syn::parse::Nothing>()?;
            }
            if attribute.path().is_ident("branch") {
                Kind::Branch
            } else {
                Kind::Output
            }
        };
        let mut parameters = Vec::new();
        for argument in &signature.inputs {
            let FnArg::Typed(argument) = argument else {
                return Err(syn::Error::new_spanned(
                    argument,
                    "workflow methods do not take self",
                ));
            };
            let Pat::Ident(pattern) = &*argument.pat else {
                return Err(syn::Error::new_spanned(
                    argument,
                    "dependency parameters are plain producer names",
                ));
            };
            let optional = schema::inner(&argument.ty, "Option").is_some();
            let borrowed = schema::inner(&argument.ty, "Option").unwrap_or(&argument.ty);
            let Type::Reference(reference) = borrowed else {
                return Err(syn::Error::new_spanned(
                    &argument.ty,
                    "a dependency is &T, &[T], Option<&T> or Option<&[T]>",
                ));
            };
            if reference.mutability.is_some() {
                return Err(syn::Error::new_spanned(
                    reference,
                    "workflow dependencies are immutable",
                ));
            }
            concrete(&reference.elem)?;
            let owned = match &*reference.elem {
                Type::Slice(slice) => {
                    let ty = &slice.elem;
                    syn::parse_quote!(::berm_lang::__private::Vec<#ty>)
                }
                Type::Path(path) if path.path.is_ident("str") => {
                    syn::parse_quote!(::berm_lang::__private::String)
                }
                ty => ty.clone(),
            };
            parameters.push(Parameter {
                name: pattern.ident.unraw().to_string(),
                owned,
                optional,
                borrowed: (*reference.elem).clone(),
            });
        }
        let name = signature.ident.unraw().to_string();
        method.attrs.retain(|attr| {
            !["step", "branch", "output"]
                .iter()
                .any(|name| attr.path().is_ident(name))
        });
        Ok(Self {
            method: method.clone(),
            name,
            kind,
            result,
            fallible,
            parameters,
        })
    }

    fn value_type(&self) -> TokenStream {
        match &self.kind {
            Kind::Once(call) | Kind::Optional(call) => {
                quote!(<#call as ::berm_lang::workflow::Call>::Output)
            }
            Kind::Each { call, .. } => {
                quote!(::berm_lang::__private::Vec<<#call as ::berm_lang::workflow::Call>::Output>)
            }
            Kind::Branch | Kind::Output => {
                let ty = &self.result;
                quote!(#ty)
            }
        }
    }
}

fn concrete(ty: &Type) -> syn::Result<()> {
    match ty {
        Type::ImplTrait(_) | Type::Infer(_) => {
            return Err(syn::Error::new_spanned(
                ty,
                "workflow signatures use concrete types, not impl Trait or inferred types",
            ));
        }
        Type::Path(path) => {
            for segment in &path.path.segments {
                if let PathArguments::AngleBracketed(arguments) = &segment.arguments {
                    for argument in &arguments.args {
                        if let GenericArgument::Type(ty) = argument {
                            concrete(ty)?;
                        }
                    }
                }
            }
        }
        Type::Reference(reference) => concrete(&reference.elem)?,
        Type::Slice(slice) => concrete(&slice.elem)?,
        Type::Array(array) => concrete(&array.elem)?,
        Type::Tuple(tuple) => {
            for ty in &tuple.elems {
                concrete(ty)?;
            }
        }
        Type::Paren(paren) => concrete(&paren.elem)?,
        _ => {}
    }
    Ok(())
}
