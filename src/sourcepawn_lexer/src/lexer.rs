use lazy_static::lazy_static;
use logos::{Lexer, Logos};
use lsp_types::{Position, Range};
use regex::Regex;

use crate::{token::Token, token_kind::TokenKind, Comment, Literal, PreprocDir};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Delta {
    pub line: i32,
    pub col: i32,
}

#[derive(Debug, Clone, Eq)]
pub struct Symbol {
    pub token_kind: TokenKind,
    text: Option<String>,
    pub range: Range,
    pub delta: Delta,
}

impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        self.token_kind == other.token_kind
            && self.text() == other.text()
            && self.range == other.range
            && self.delta == other.delta
    }
}

impl Symbol {
    pub fn new(token_kind: TokenKind, text: Option<&str>, range: Range, delta: Delta) -> Self {
        Self {
            token_kind,
            text: text.map(|s| s.to_string()),
            range,
            delta,
        }
    }

    pub fn text(&self) -> String {
        match &self.token_kind {
            TokenKind::Operator(op) => return op.text(),
            TokenKind::PreprocDir(dir) => {
                if matches!(
                    self.token_kind,
                    TokenKind::PreprocDir(PreprocDir::MPragma)
                        | TokenKind::PreprocDir(PreprocDir::MInclude)
                        | TokenKind::PreprocDir(PreprocDir::MTryinclude)
                ) {
                    return self.text.clone().unwrap();
                }
                return dir.text();
            }
            TokenKind::Comment(_) | TokenKind::Literal(_) | TokenKind::Identifier => {
                return self.text.clone().unwrap()
            }
            TokenKind::Newline => "\n",
            TokenKind::LineContinuation => "\\\n",
            TokenKind::Bool => "bool",
            TokenKind::Break => "break",
            TokenKind::Case => "case",
            TokenKind::Char => "char",
            TokenKind::Class => "class",
            TokenKind::Const => "const",
            TokenKind::Continue => "continue",
            TokenKind::Decl => "decl",
            TokenKind::Default => "default",
            TokenKind::Defined => "defined",
            TokenKind::Delete => "delete",
            TokenKind::Do => "do",
            TokenKind::Else => "else",
            TokenKind::Enum => "enum",
            TokenKind::False => "false",
            TokenKind::Float => "float",
            TokenKind::For => "for",
            TokenKind::Forward => "forward",
            TokenKind::Functag => "functag",
            TokenKind::Function => "function",
            TokenKind::If => "if",
            TokenKind::Int => "int",
            TokenKind::InvalidFunction => "INVALID_FUNCTION",
            TokenKind::Methodmap => "methodmap",
            TokenKind::Native => "native",
            TokenKind::Null => "null",
            TokenKind::New => "new",
            TokenKind::Object => "object",
            TokenKind::Property => "property",
            TokenKind::Public => "public",
            TokenKind::Return => "return",
            TokenKind::Sizeof => "sizeof",
            TokenKind::Static => "static",
            TokenKind::Stock => "stock",
            TokenKind::Struct => "struct",
            TokenKind::Switch => "switch",
            TokenKind::This => "this",
            TokenKind::True => "true",
            TokenKind::Typedef => "typedef",
            TokenKind::Typeset => "typeset",
            TokenKind::Union => "union",
            TokenKind::Using => "using",
            TokenKind::ViewAs => "view_as",
            TokenKind::Void => "void",
            TokenKind::While => "while",
            TokenKind::Nullable => "__nullable__",
            TokenKind::Intrinsics => "__intrinsics__",
            TokenKind::Semicolon => ";",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            TokenKind::Comma => ",",
            TokenKind::Qmark => "?",
            TokenKind::Colon => ":",
            TokenKind::Scope => "::",
            TokenKind::Dot => ".",
            TokenKind::Unknown => "",
            TokenKind::Eof => "\0",
        }
        .to_string()
    }

    pub fn to_int(&self) -> Option<u32> {
        if let TokenKind::Literal(lit) = &self.token_kind {
            return lit.to_int(&self.text());
        }

        None
    }

    pub fn inline_text(&self) -> String {
        let text = self.text();
        match &self.token_kind {
            TokenKind::Literal(lit) => match lit {
                Literal::StringLiteral | Literal::CharLiteral => {
                    return text.replace("\\\n", "").replace("\\\r\n", "")
                }
                _ => (),
            },
            TokenKind::Comment(com) => {
                if *com == Comment::BlockComment {
                    return text.replace('\n', "").replace("\r\n", "");
                }
            }
            TokenKind::PreprocDir(dir) => {
                if matches!(
                    *dir,
                    PreprocDir::MPragma | PreprocDir::MInclude | PreprocDir::MTryinclude
                ) {
                    return text.replace("\\\n", "").replace("\\\r\n", "");
                }
            }
            _ => (),
        }

        text
    }
}

#[derive(Debug, Clone)]
pub struct SourcepawnLexer<'a> {
    lexer: Lexer<'a, Token>,
    line_number: u32,
    line_span_start: u32,
    in_preprocessor: bool,
    prev_range: Option<Range>,
    eof: bool,
}

impl SourcepawnLexer<'_> {
    pub fn new(input: &str) -> SourcepawnLexer {
        SourcepawnLexer {
            lexer: Token::lexer(input),
            line_number: 0,
            line_span_start: 0,
            in_preprocessor: false,
            prev_range: None,
            eof: false,
        }
    }

    pub fn in_preprocessor(&self) -> bool {
        self.in_preprocessor && !self.eof
    }

    fn delta(&mut self, range: &Range) -> Delta {
        let delta = if let Some(prev_range) = self.prev_range {
            Delta {
                line: (range.start.line as i32 - prev_range.end.line as i32),
                col: (range.start.character as i32 - prev_range.end.character as i32),
            }
        } else {
            Delta::default()
        };
        self.prev_range = Some(*range);

        delta
    }
}

impl Iterator for SourcepawnLexer<'_> {
    type Item = Symbol;

    fn next(&mut self) -> Option<Symbol> {
        lazy_static! {
            static ref RE1: Regex = Regex::new(r"\n").unwrap();
        }
        lazy_static! {
            static ref RE2: Regex = Regex::new(r"\\\r?\n").unwrap();
        }
        let token = self.lexer.next();
        if token.is_none() && !self.eof {
            // Reached EOF
            self.eof = true;
            let range = Range::new(
                Position::new(
                    self.line_number,
                    self.lexer.source().len() as u32 - self.line_span_start,
                ),
                Position::new(
                    self.line_number,
                    self.lexer.source().len() as u32 - self.line_span_start,
                ),
            );
            return Some(Symbol {
                token_kind: TokenKind::Eof,
                text: None,
                range,
                delta: self.delta(&range),
            });
        }
        let token = token?;

        let start_line = self.line_number;
        let start_col = self.lexer.span().start as u32 - self.line_span_start;
        let text = match token {
            Token::Identifier
            | Token::IntegerLiteral
            | Token::HexLiteral
            | Token::BinaryLiteral
            | Token::OctodecimalLiteral
            | Token::StringLiteral
            | Token::CharLiteral
            | Token::FloatLiteral
            | Token::BlockComment
            | Token::LineComment
            | Token::MPragma
            | Token::MInclude
            | Token::MTryinclude => Some(self.lexer.slice().to_string()),
            _ => None,
        };

        match token {
            Token::StringLiteral
            | Token::BlockComment
            | Token::MPragma
            | Token::MInclude
            | Token::MTryinclude => {
                if matches!(token, Token::MPragma | Token::MInclude | Token::MTryinclude) {
                    self.in_preprocessor = true;
                }
                // Safe unwrap here as those tokens have text.
                let text = text.clone().unwrap();
                let line_breaks: Vec<_> = RE1.find_iter(text.as_str()).collect();
                let line_continuations: Vec<_> = RE2.find_iter(text.as_str()).collect();

                if let Some(last) = line_continuations.last() {
                    self.line_number += line_breaks.len() as u32;
                    self.line_span_start = (self.lexer.span().start + last.end()) as u32;
                } else if let Some(last) = line_breaks.last() {
                    self.in_preprocessor = false;
                    self.line_number += line_breaks.len() as u32;
                    self.line_span_start = (self.lexer.span().start + last.start()) as u32;
                }
            }
            Token::MDefine
            | Token::MDeprecate
            | Token::MIf
            | Token::MElse
            | Token::MElseif
            | Token::MEndinput
            | Token::MFile
            | Token::MOptionalNewdecls
            | Token::MOptionalSemi
            | Token::MRequireNewdecls
            | Token::MRequireSemi
            | Token::MUndef
            | Token::MEndif
            | Token::MLeaving => self.in_preprocessor = true,
            Token::LineContinuation => {
                self.line_number += 1;
                self.line_span_start = self.lexer.span().end as u32;
            }
            Token::Newline => {
                self.in_preprocessor = false;
                self.line_number += 1;
                self.line_span_start = self.lexer.span().end as u32;
            }
            _ => {}
        }
        let token_kind = TokenKind::try_from(token).ok()?;
        let range = Range::new(
            Position::new(start_line, start_col),
            Position::new(
                self.line_number,
                self.lexer.span().end as u32 - self.line_span_start,
            ),
        );
        Some(Symbol {
            token_kind,
            text,
            range,
            delta: self.delta(&range),
        })
    }
}
