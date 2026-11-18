//! Shader effects and `.swfx` shader configs (DESIGN.md §2-§3, §6).
//!
//! An effect is HLSL passes plus declared params; a config lists effect instances with param values.
//! This crate has no store or game access: the launcher, the CLI and the preview all use it.

use std::collections::BTreeMap;
use std::ops::Range;

use serde::{Deserialize, Serialize};

mod compile;
mod config;
mod editor;
mod effect;
mod lint;
mod pack;
pub mod rules;
pub mod signals;
mod text;
mod when;

pub use compile::{CompileError, CompiledEffect, PassEntry, compile, compile_pass};
pub use editor::{completions, hover, tokens};
pub use effect::{params_hlsl, parse_effect};
pub use lint::lint;
pub use pack::{pack, resolve, resolve_rules};
pub use rules::Rule;

/// The most float slots one effect's params may take (`option(0..31)`).
pub const MAX_SLOTS: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    Number(f64),
    Text(String),
    Vector(Vec<f64>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamKind {
    Float {
        min: Option<f64>,
        max: Option<f64>,
        step: Option<f64>,
    },
    Int {
        min: Option<i64>,
        max: Option<i64>,
    },
    Bool,
    Enum {
        values: Vec<String>,
    },
    Float2 {
        min: Option<f64>,
        max: Option<f64>,
    },
    Color3 {
        min: Option<f64>,
        max: Option<f64>,
    },
    Color4 {
        min: Option<f64>,
        max: Option<f64>,
    },
}

impl ParamKind {
    /// The float slots a param of this kind takes.
    pub fn width(&self) -> usize {
        match self {
            Self::Float2 { .. } => 2,
            Self::Color3 { .. } => 3,
            Self::Color4 { .. } => 4,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub id: String,
    pub label: String,
    pub doc: String,
    pub kind: ParamKind,
    pub default: Value,
    pub slot: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextureDecl {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub persistent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mips: Option<u32>,
}

/// The most mip levels a texture may declare.
pub const MAX_MIPS: u32 = 12;

/// The levels a texture of this size can carry: the declared count, at least 1, at most `MAX_MIPS` and
/// floor(log2(max(w, h))) + 1.
pub fn mip_levels(width: u32, height: u32, declared: u32) -> u32 {
    let size = width.max(height).max(1);
    declared.clamp(1, MAX_MIPS.min(32 - size.leading_zeros()))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassDecl {
    pub name: String,
    pub shader: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub raw: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Origin {
    Library,
    Addon { id: String, name: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    pub id: String,
    pub name: String,
    pub description: String,
    pub params: Vec<Param>,
    pub textures: Vec<TextureDecl>,
    pub passes: Vec<PassDecl>,
    /// Float slots the params take.
    pub slots: usize,
    pub origin: Origin,
    /// Things the checker fixed up while parsing.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub effects: Vec<Effect>,
}

impl Catalog {
    pub fn get(&self, id: &str) -> Option<&Effect> {
        self.effects.iter().find(|e| e.id == id)
    }

    /// The effect id closest to `id`, for a "did you mean" fix.
    pub fn did_you_mean(&self, id: &str) -> Option<&str> {
        text::did_you_mean(id, self.effects.iter().map(|e| e.id.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub effect: String,
    pub label: Option<String>,
    pub enabled: bool,
    pub values: BTreeMap<String, Value>,
    /// The instance's table in the text, header to last line.
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CondOp {
    True,
    Is { values: Vec<String>, wheels: u8 },
    Above(f64),
    Below(f64),
    Between(f64, f64),
    Event,
}

/// One condition of a rule: a signal and what it must do.
#[derive(Debug, Clone, PartialEq)]
pub struct Cond {
    pub signal: String,
    pub op: CondOp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ramp {
    pub signal: String,
    pub from: f64,
    pub to: f64,
}

/// A `[when.set.<effect>]` table: the overrides one rule applies to one instance.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleSet {
    /// Index into [`Config::instances`].
    pub instance: usize,
    pub values: BTreeMap<String, Value>,
    /// `enabled = false`: the rule turns the instance off.
    pub off: bool,
    pub span: Range<usize>,
}

/// A parsed `[[when]]` rule with its defaults filled in.
#[derive(Debug, Clone, PartialEq)]
pub struct WhenRule {
    pub name: Option<String>,
    pub priority: i32,
    /// Position in the file.
    pub order: usize,
    pub event: bool,
    pub when: Vec<Cond>,
    pub ramp: Option<Ramp>,
    pub min_ms: f64,
    pub hold_ms: f64,
    pub for_ms: f64,
    pub fade_in_ms: f64,
    pub fade_out_ms: f64,
    pub sets: Vec<RuleSet>,
    /// The rule's tables in the text, `[[when]]` to the last line of its last `set`.
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Config {
    pub name: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub instances: Vec<Instance>,
    pub rules: Vec<WhenRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A position for the editor: 0-based line and UTF-16 column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fix {
    pub title: String,
    pub range: Range<usize>,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub range: Range<usize>,
    pub start: Pos,
    pub end: Pos,
    pub message: String,
    pub fix: Option<Fix>,
}

#[derive(Debug, Clone, Default)]
pub struct Lint {
    /// `None` when any diagnostic is an error.
    pub config: Option<Config>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Lint {
    pub fn errors(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Effect,
    Param,
    Value,
    Keyword,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub doc: String,
    pub range: Range<usize>,
    pub insert: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hover {
    /// Markdown.
    pub text: String,
    pub range: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Comment,
    Header,
    Key,
    Number,
    String,
    Bool,
    Punct,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveInstance {
    pub effect: String,
    #[serde(default)]
    pub label: String,
    pub params: Vec<f32>,
}

/// `active.json` (DESIGN.md §5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Active {
    pub version: u32,
    pub serial: u64,
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub forced_by: String,
    pub toggle_key: String,
    pub instances: Vec<ActiveInstance>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Dxil,
    Spirv,
}

/// Dxc's profile for passes.
pub const PROFILE: &str = "cs_6_0";
