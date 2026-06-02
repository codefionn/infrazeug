//! Compile-time `template!` macro for infrazeug.
//!
//! Renders an inline, Rust-native template string to a `String` at the call
//! site. Because every interpolation and control-flow header is emitted as
//! ordinary Rust (referencing in-scope bindings), **rustc type-checks the
//! template**: `{{ v.nope }}` against a struct without that field is a compile
//! error, not a runtime miss.
//!
//! # Syntax
//! - `{{ expr }}` — interpolate any `Display` expression. Filters are plain
//!   method calls: `{{ name.to_uppercase() }}`.
//! - `@for pat in expr { body }` — real Rust `for` loop.
//! - `@if expr { body } @else if expr { body } @else { body }` — real Rust `if`.
//! - Escapes: `@@` → `@`, `@{` → `{`, `@}` → `}`.
//!
//! A bare `}` ends the nearest control-flow body; literal `}` in body text must
//! be written `@}`. Nested control flow consumes its own braces, so no brace
//! counting is needed.
//!
//! # Forms
//! ```ignore
//! template!("port = {{ port }}\n")                 // uses in-scope `port`
//! template!("@for h in &hosts { {{ h.ip }}\n }", hosts = cfg.hosts)  // named bindings
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream, Parser as _};
use syn::{Expr, Ident, LitStr, Token};

struct TemplateInput {
    src: LitStr,
    bindings: Vec<(Ident, Expr)>,
}

impl Parse for TemplateInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let src: LitStr = input.parse()?;
        let mut bindings = Vec::new();
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break; // trailing comma
            }
            let name: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let expr: Expr = input.parse()?;
            bindings.push((name, expr));
        }
        Ok(Self { src, bindings })
    }
}

/// Parsed template AST node.
enum Node {
    Text(String),
    /// Raw Rust expression source from `{{ … }}`.
    Expr(String),
    For {
        pat: String,
        iter: String,
        body: Vec<Node>,
    },
    If {
        /// Each branch: `Some(cond)` for `if`/`else if`, `None` for the final `else`.
        branches: Vec<(Option<String>, Vec<Node>)>,
    },
}

struct Scanner {
    chars: Vec<char>,
    pos: usize,
}

impl Scanner {
    fn new(s: &str) -> Self {
        Self {
            chars: s.chars().collect(),
            pos: 0,
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn starts_with(&self, s: &str) -> bool {
        let mut i = self.pos;
        for ch in s.chars() {
            match self.chars.get(i) {
                Some(c) if *c == ch => i += 1,
                _ => return false,
            }
        }
        true
    }

    /// True when the keyword `kw` appears at `pos` followed by a word boundary.
    fn starts_keyword(&self, kw: &str) -> bool {
        if !self.starts_with(kw) {
            return false;
        }
        match self.chars.get(self.pos + kw.chars().count()) {
            None => true,
            Some(c) => !c.is_alphanumeric() && *c != '_',
        }
    }

    fn advance(&mut self, n: usize) {
        self.pos += n;
    }

    /// Collect raw source until the unescaped delimiter `delim`, consuming it.
    fn take_until(&mut self, delim: &str) -> Result<String, String> {
        let mut out = String::new();
        while !self.eof() {
            if self.starts_with(delim) {
                self.advance(delim.chars().count());
                return Ok(out);
            }
            out.push(self.chars[self.pos]);
            self.pos += 1;
        }
        Err(format!("unterminated, expected `{delim}`"))
    }

    /// Collect raw source until a top-level `{`, consuming the `{`.
    fn take_header(&mut self) -> Result<String, String> {
        let mut out = String::new();
        while !self.eof() {
            if self.chars[self.pos] == '{' {
                self.pos += 1;
                return Ok(out);
            }
            out.push(self.chars[self.pos]);
            self.pos += 1;
        }
        Err("expected `{` to open block body".to_string())
    }
}

/// Parse template nodes. When `in_block`, stop at (and consume) the closing `}`.
fn parse_nodes(sc: &mut Scanner, in_block: bool) -> Result<Vec<Node>, String> {
    let mut nodes = Vec::new();
    let mut text = String::new();

    macro_rules! flush {
        () => {
            if !text.is_empty() {
                nodes.push(Node::Text(std::mem::take(&mut text)));
            }
        };
    }

    loop {
        if sc.eof() {
            if in_block {
                return Err("unexpected end of template, expected `}`".to_string());
            }
            break;
        }
        let c = sc.chars[sc.pos];
        if in_block && c == '}' {
            sc.advance(1);
            flush!();
            return Ok(nodes);
        }
        if sc.starts_with("{{") {
            flush!();
            sc.advance(2);
            let expr = sc.take_until("}}")?;
            nodes.push(Node::Expr(expr.trim().to_string()));
        } else if sc.starts_with("@@") {
            text.push('@');
            sc.advance(2);
        } else if sc.starts_with("@{") {
            text.push('{');
            sc.advance(2);
        } else if sc.starts_with("@}") {
            text.push('}');
            sc.advance(2);
        } else if sc.starts_keyword("@for") {
            flush!();
            sc.advance(4);
            nodes.push(parse_for(sc)?);
        } else if sc.starts_keyword("@if") {
            flush!();
            sc.advance(3);
            nodes.push(parse_if(sc)?);
        } else if sc.starts_keyword("@else") {
            return Err("`@else` without a matching `@if`".to_string());
        } else {
            text.push(c);
            sc.advance(1);
        }
    }
    flush!();
    Ok(nodes)
}

fn parse_for(sc: &mut Scanner) -> Result<Node, String> {
    let header = sc.take_header()?;
    let (pat, iter) = split_for_header(&header)?;
    let body = parse_nodes(sc, true)?;
    Ok(Node::For { pat, iter, body })
}

/// Split `PAT in EXPR` at the first top-level ` in ` keyword.
fn split_for_header(header: &str) -> Result<(String, String), String> {
    let needle = " in ";
    let idx = header
        .find(needle)
        .ok_or_else(|| format!("`@for` header `{}` is missing `in`", header.trim()))?;
    let pat = header[..idx].trim().to_string();
    let iter = header[idx + needle.len()..].trim().to_string();
    if pat.is_empty() || iter.is_empty() {
        return Err(format!("malformed `@for` header `{}`", header.trim()));
    }
    Ok((pat, iter))
}

fn parse_if(sc: &mut Scanner) -> Result<Node, String> {
    let mut branches = Vec::new();
    let cond = sc.take_header()?.trim().to_string();
    if cond.is_empty() {
        return Err("`@if` is missing a condition".to_string());
    }
    let body = parse_nodes(sc, true)?;
    branches.push((Some(cond), body));

    loop {
        // Peek (skipping whitespace) for `@else`; only consume if present.
        let save = sc.pos;
        while !sc.eof() && sc.chars[sc.pos].is_whitespace() {
            sc.pos += 1;
        }
        if sc.starts_keyword("@else") {
            sc.advance(5);
            // skip whitespace after @else
            while !sc.eof() && sc.chars[sc.pos].is_whitespace() {
                sc.pos += 1;
            }
            if sc.starts_keyword("@if") || sc.starts_keyword("if") {
                // `@else if` (accept either `@else @if` or `@else if`)
                if sc.starts_keyword("@if") {
                    sc.advance(3);
                } else {
                    sc.advance(2);
                }
                let cond = sc.take_header()?.trim().to_string();
                if cond.is_empty() {
                    return Err("`@else if` is missing a condition".to_string());
                }
                let body = parse_nodes(sc, true)?;
                branches.push((Some(cond), body));
            } else {
                // plain `@else { … }`
                let header = sc.take_header()?;
                if !header.trim().is_empty() {
                    return Err(format!(
                        "unexpected text after `@else`: `{}`",
                        header.trim()
                    ));
                }
                let body = parse_nodes(sc, true)?;
                branches.push((None, body));
                break;
            }
        } else {
            sc.pos = save; // not an else — leave whitespace for parent text
            break;
        }
    }
    Ok(Node::If { branches })
}

fn codegen(nodes: &[Node], span: proc_macro2::Span) -> Result<TokenStream2, String> {
    let mut ts = TokenStream2::new();
    for node in nodes {
        let piece = match node {
            Node::Text(s) => {
                let lit = LitStr::new(s, span);
                quote! { __izg_out.push_str(#lit); }
            }
            Node::Expr(src) => {
                let expr: Expr =
                    syn::parse_str(src).map_err(|e| format!("invalid expression `{src}`: {e}"))?;
                quote! {
                    ::core::fmt::Write::write_fmt(
                        &mut __izg_out,
                        ::core::format_args!("{}", &(#expr)),
                    )
                    .expect("infrazeug template: writing to String is infallible");
                }
            }
            Node::For { pat, iter, body } => {
                let pat_tok = syn::Pat::parse_single
                    .parse_str(pat)
                    .map_err(|e| format!("invalid `@for` pattern `{pat}`: {e}"))?;
                let iter_tok: Expr = syn::parse_str(iter)
                    .map_err(|e| format!("invalid `@for` iterator `{iter}`: {e}"))?;
                let body_ts = codegen(body, span)?;
                quote! { for #pat_tok in #iter_tok { #body_ts } }
            }
            Node::If { branches } => codegen_if(branches, span)?,
        };
        ts.extend(piece);
    }
    Ok(ts)
}

fn codegen_if(
    branches: &[(Option<String>, Vec<Node>)],
    span: proc_macro2::Span,
) -> Result<TokenStream2, String> {
    let mut out = TokenStream2::new();
    for (i, (cond, body)) in branches.iter().enumerate() {
        let body_ts = codegen(body, span)?;
        match cond {
            Some(c) => {
                let cond_tok: Expr =
                    syn::parse_str(c).map_err(|e| format!("invalid `@if` condition `{c}`: {e}"))?;
                if i == 0 {
                    out.extend(quote! { if #cond_tok { #body_ts } });
                } else {
                    out.extend(quote! { else if #cond_tok { #body_ts } });
                }
            }
            None => out.extend(quote! { else { #body_ts } }),
        }
    }
    Ok(out)
}

/// Render an inline Rust-native template to a `String` at compile time.
///
/// See the crate docs for syntax. Every embedded expression is type-checked by
/// rustc against in-scope bindings (or the `name = expr` bindings passed after
/// the template string).
#[proc_macro]
pub fn template(input: TokenStream) -> TokenStream {
    let TemplateInput { src, bindings } = syn::parse_macro_input!(input as TemplateInput);
    let span = src.span();

    let mut sc = Scanner::new(&src.value());
    let nodes = match parse_nodes(&mut sc, false) {
        Ok(n) => n,
        Err(msg) => return syn::Error::new(span, msg).to_compile_error().into(),
    };
    let body = match codegen(&nodes, span) {
        Ok(ts) => ts,
        Err(msg) => return syn::Error::new(span, msg).to_compile_error().into(),
    };

    let lets = bindings.iter().map(|(name, expr)| {
        quote! { let #name = #expr; }
    });

    let expanded = quote! {
        {
            let mut __izg_out = ::std::string::String::new();
            #(#lets)*
            #body
            __izg_out
        }
    };
    expanded.into()
}
