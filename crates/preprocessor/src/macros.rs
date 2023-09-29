use std::{cmp::Ordering, collections::VecDeque};

use fxhash::FxHashMap;
use lsp_types::{Position, Range};
use sourcepawn_lexer::{Literal, Operator, Symbol, TokenKind};

use super::errors::{ExpansionError, MacroNotFoundError, ParseIntError};
use crate::Macro;

/// Arguments of a [macro](Macro) call.
type MacroArguments = [Vec<Symbol>; 10];

/// Queue of symbols and the delta before the previous symbol in the expansion stack.
///
/// The delta can be different from the symbol's delta, especially for nested macro calls.
type MacroContext = VecDeque<QueuedSymbol>;

/// Representation of a queued [symbol](Symbol) after the expansion of a macro.
#[derive(Debug, Clone)]
struct QueuedSymbol {
    /// Queued [symbol](Symbol).
    symbol: Symbol,

    /// [Delta](sourcepawn_lexer::Delta) of the queued [symbol](Symbol) (which can be different than
    /// the [symbol](Symbol)'s [delta](sourcepawn_lexer::Delta)).
    delta: sourcepawn_lexer::Delta,
}

impl QueuedSymbol {
    pub fn new(symbol: Symbol, delta: sourcepawn_lexer::Delta) -> Self {
        Self { symbol, delta }
    }
}

/// Try to expand an identifier and return a [vector][Vec] of expanded [symbols](Symbol).
///
/// We use a [context](MacroContext) stack to keep track of the expanded macros.
/// The stack is initialized with the identifier we encountered before we called this function.
/// For each loop, we pop the stack and then pop the [symbol](Symbol) queue until it's empty.
/// If the symbol we popped is an identifier, we have reached an (inner) macro call.
/// The macro is expanded, we generate a new [context](MacroContext) and push the current
/// [context](MacroContext) then the new [context](MacroContext) on the stack.
///
/// If a popped [context](MacroContext) is empty, it is removed from the stack.
///
/// See [the GNU documentation on macro expansion](https://gcc.gnu.org/onlinedocs/cppinternals/Macro-Expansion.html#Macro-expansion-overview) for more details.
///
/// # Arguments
///
/// * `lexer` - [SourcepawnLexer](sourcepawn_lexer::lexer) to iterate over.
/// * `macros` - Known macros.
/// * `symbol` - Identifier [symbol](Symbol) to expand.
/// * `expansion_stack` - Expansion stack used instead of the lexer if it is not empty.
/// * `allow_undefined_macros` - Should not found macros throw an error.
pub(super) fn expand_identifier<T>(
    lexer: &mut T,
    macros: &FxHashMap<String, Macro>,
    symbol: &Symbol,
    expansion_stack: &mut Vec<Symbol>,
    allow_undefined_macros: bool,
) -> Result<Vec<Symbol>, ExpansionError>
where
    T: Iterator<Item = Symbol>,
{
    let mut expanded_macros = Vec::new();
    let mut reversed_expansion_stack = Vec::new();
    let mut context_stack = vec![VecDeque::from([QueuedSymbol::new(
        symbol.clone(),
        symbol.delta,
    )])];
    while !context_stack.is_empty() && context_stack.len() < 6 {
        let mut current_context = context_stack.pop().unwrap();
        let Some(queued_symbol) = current_context.pop_front() else {
            continue;
        };
        match &queued_symbol.symbol.token_kind {
            TokenKind::Identifier => {
                let macro_ = match macros.get(&queued_symbol.symbol.text()) {
                    Some(m) => m,
                    None => {
                        if !allow_undefined_macros {
                            return Err(MacroNotFoundError::new(
                                queued_symbol.symbol.text(),
                                queued_symbol.symbol.range,
                            )
                            .into());
                        }
                        let mut symbol = queued_symbol.symbol.clone();
                        symbol.delta = queued_symbol.delta;
                        reversed_expansion_stack.push(symbol);
                        context_stack.push(current_context);
                        continue;
                    }
                };
                if context_stack.is_empty() {
                    // Do not keep track of sub-macros, they will not appear in the final document.
                    expanded_macros.push(queued_symbol.symbol.clone());
                }
                let new_context = if macro_.params.is_none() {
                    expand_non_macro_define(macro_, &queued_symbol.symbol.delta)
                } else {
                    let args =
                        collect_arguments(lexer, &mut current_context, macro_.nb_params as usize);
                    expand_macro(args, macro_, &queued_symbol.symbol)?
                };
                context_stack.push(current_context);
                context_stack.push(new_context);
            }
            TokenKind::Literal(Literal::StringLiteral)
            | TokenKind::Literal(Literal::CharLiteral) => {
                let text = &queued_symbol.symbol.inline_text();
                reversed_expansion_stack.push(Symbol::new(
                    queued_symbol.symbol.token_kind.clone(),
                    Some(text),
                    Range::new(
                        queued_symbol.symbol.range.start,
                        Position::new(
                            queued_symbol.symbol.range.start.line,
                            queued_symbol.symbol.range.start.character + text.len() as u32,
                        ),
                    ),
                    queued_symbol.symbol.delta,
                ));
                context_stack.push(current_context);
            }
            TokenKind::Newline | TokenKind::LineContinuation | TokenKind::Comment(_) => {
                context_stack.push(current_context);
            }
            _ => {
                let mut symbol = queued_symbol.symbol.clone();
                symbol.delta = queued_symbol.delta;
                reversed_expansion_stack.push(symbol);
                context_stack.push(current_context);
            }
        }
    }

    // The expansion stack expects [symbols](Symbol) to be in reverse order and this algorithm
    // produces them in the correct order, therefore we have to reverse them.
    expansion_stack.extend(reversed_expansion_stack.into_iter().rev());

    Ok(expanded_macros)
}

/// Expand a non macro define by returning a new [context](MacroContext) of all the [symbols](Symbol)
/// in the [macro](Macro)'s body.
///
/// # Arguments
///
/// * `macro_` - [Macro] we are expanding.
/// * `delta` - [Delta](sourcepawn_lexer::Delta) of the [symbols](Symbol) we are expanding
/// to use for the first symbol in the [macro's](Macro) body.
fn expand_non_macro_define(macro_: &Macro, delta: &sourcepawn_lexer::Delta) -> MacroContext {
    let macro_context = macro_
        .body
        .iter()
        .enumerate()
        .map(|(i, child)| QueuedSymbol {
            symbol: child.clone(),
            delta: if i == 0 { *delta } else { child.delta },
        })
        .collect();

    macro_context
}

/// Expand a function like macro by returning a new [context](MacroContext) of all the [symbols](Symbol)
/// in the [macro](Macro)'s body.
///
/// # Arguments
///
/// * `args` - [Arguments](MacroArguments) of the macro call.
/// * `macro_` - [Macro] we are expanding.
/// * `symbol` - [Symbol] that originated the macro expansion. Used to keep track of the delta to insert.
fn expand_macro(
    args: MacroArguments,
    macro_: &Macro,
    symbol: &Symbol,
) -> Result<MacroContext, ParseIntError> {
    let mut new_context = MacroContext::default();
    let mut consecutive_percent = 0;
    let mut stringize_delta = None;
    for (i, child) in macro_.body.iter().enumerate() {
        match &child.token_kind {
            TokenKind::Operator(Operator::Percent) => {
                // Count consecutive % tokens.
                // Keep every odd number and if a literal is found, pop the stack to remove it
                // and insert the argument instead.
                // This allows to preserve the spacing between the last token and the % when
                // there is an escaped %.
                consecutive_percent += 1;
                if consecutive_percent % 2 == 1 {
                    new_context.push_back(QueuedSymbol::new(child.clone(), child.delta))
                }
            }
            TokenKind::Operator(Operator::Stringize) => {
                stringize_delta = Some(child.delta);
                new_context.push_back(QueuedSymbol::new(child.clone(), child.delta))
            }
            TokenKind::Literal(Literal::IntegerLiteral) => {
                if consecutive_percent == 1 {
                    let percent_symbol = new_context.pop_back().unwrap(); // Safe unwrap.
                    let arg_idx = child
                        .to_int()
                        .ok_or_else(|| ParseIntError::new(child.text(), child.range))?
                        as usize;
                    if arg_idx >= 10 {
                        return Err(ParseIntError::new(child.text(), child.range));
                    }
                    // Safe to unwrap here because we know the macro has arguments.
                    let arg_idx = macro_.params.as_ref().unwrap()[arg_idx] as usize;
                    if arg_idx >= 10 {
                        return Err(ParseIntError::new(child.text(), child.range));
                    }
                    if let Some(stringize_delta) = stringize_delta.take() {
                        new_context.pop_back();
                        let mut stringized = '"'.to_string();
                        for (j, sub_child) in args[arg_idx].iter().enumerate() {
                            if j > 0 && sub_child.delta.col > 0 {
                                stringized.push_str(&" ".repeat(sub_child.delta.col as usize));
                            }
                            stringized.push_str(&sub_child.inline_text());
                        }
                        stringized.push('"');
                        let delta = if i == 2 {
                            symbol.delta
                        } else {
                            stringize_delta
                        };
                        let symbol = Symbol::new(
                            TokenKind::Literal(Literal::StringLiteral),
                            Some(&stringized),
                            Range::new(
                                symbol.range.start,
                                Position::new(
                                    symbol.range.start.line,
                                    symbol.range.start.character + stringized.len() as u32,
                                ),
                            ),
                            delta,
                        );
                        new_context.push_back(QueuedSymbol::new(symbol, delta));
                    } else {
                        for (j, sub_child) in args[arg_idx].iter().enumerate() {
                            new_context.push_back(QueuedSymbol::new(
                                sub_child.clone(),
                                if i == 1 {
                                    symbol.delta
                                } else if j == 0 {
                                    percent_symbol.delta
                                } else {
                                    sub_child.delta
                                },
                            ));
                        }
                    }
                } else {
                    new_context.push_back(QueuedSymbol::new(child.clone(), child.delta));
                }
                consecutive_percent = 0;
            }
            _ => {
                new_context.push_back(QueuedSymbol::new(
                    child.clone(),
                    if i == 0 { symbol.delta } else { child.delta },
                ));
                consecutive_percent = 0;
                stringize_delta = None;
            }
        }
    }

    Ok(new_context)
}

/// Assuming we are right before a macro call in the lexer, collect the arguments
/// and store them in an array, in the order they appear in.
///
/// # Arguments
///
/// * `lexer` - [SourcepawnLexer](sourcepawn_lexer::lexer) to iterate over.
/// * `args_stack` - [Vec](Vec) of [Symbols](sourcepawn_lexer::Symbol) that represent the
/// stack of arguments that are being processed.
/// * `nb_params` - Number of parameters in the current macro.
fn collect_arguments<T>(
    lexer: &mut T,
    context: &mut MacroContext,
    nb_params: usize,
) -> MacroArguments
where
    T: Iterator<Item = Symbol>,
{
    let mut paren_depth = 0;
    let mut arg_idx: usize = 0;
    let mut args: MacroArguments = Default::default();
    while let Some(sub_symbol) = if !context.is_empty() {
        Some(context.pop_front().unwrap().symbol)
    } else {
        lexer.next()
    } {
        match &sub_symbol.token_kind {
            TokenKind::LParen => {
                paren_depth += 1;
                if paren_depth > 1 {
                    args[arg_idx].push(sub_symbol.clone())
                }
            }
            TokenKind::RParen => {
                if paren_depth > 1 {
                    args[arg_idx].push(sub_symbol.clone())
                }
                paren_depth -= 1;
                if paren_depth == 0 {
                    break;
                }
            }
            TokenKind::Comma => {
                match paren_depth.cmp(&1) {
                    Ordering::Equal => {
                        if arg_idx + 1 < nb_params {
                            arg_idx += 1;
                        } else {
                            // The stack of arguments is overflowed, store the rest in the last argument,
                            // including the comma.
                            args[arg_idx].push(sub_symbol.clone())
                        }
                    }
                    Ordering::Greater => args[arg_idx].push(sub_symbol.clone()),
                    Ordering::Less => (),
                }
            }
            _ => {
                if paren_depth > 0 {
                    args[arg_idx].push(sub_symbol.clone());
                }
            }
        }
    }

    args
}
