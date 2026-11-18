use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::base::input::{TextDecoration, TextDecorationCollection};
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{
    CodeActionProvider, CompletionProvider, Editor, EditorState, FoldRange, HighlightStyleResolver,
    HoverProvider, InputEdit, InputEvent, InputHighlighter, Position, Rope, RopeExt,
};
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use lsp_types::{
    CodeAction, CodeActionKind, CompletionContext, CompletionItem, CompletionItemKind, CompletionResponse,
    CompletionTextEdit, Documentation, HoverContents, MarkupContent, MarkupKind, TextEdit,
};
use swofty_core::shaders::ConfigEntry;
use swofty_i18n::{tr, trf, trn};
use swofty_shaders::{ActiveInstance, Catalog, CompletionKind, Config, Severity, TokenKind};

use super::{Mode, ShadersView};
use crate::theme::{Palette, ScrollArea, palette};
use crate::ui::EmptyState;
use crate::ui::tokens::radius;

const LINT_DEBOUNCE: Duration = Duration::from_millis(120);
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

pub(super) type SharedCatalog = Rc<RefCell<Arc<Catalog>>>;
pub(super) type SharedProblems = Rc<RefCell<Vec<Problem>>>;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Problem {
    pub severity: Severity,
    pub range: Range<usize>,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub fix: Option<swofty_shaders::Fix>,
}

#[derive(Default)]
pub(super) struct LintView {
    pub rev: u64,
    pub problems: Vec<Problem>,
    pub done: bool,
}

impl LintView {
    pub fn errors(&self) -> usize {
        self.problems
            .iter()
            .filter(|p| p.severity == Severity::Error)
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.problems
            .iter()
            .filter(|p| p.severity == Severity::Warning)
            .count()
    }
}

pub(super) struct EditorPane {
    pub state: Entity<EditorState>,
    pub loaded: Option<swofty_core::shaders::ConfigRef>,
    pub loading: bool,
    pub saved: String,
    pub last_text: String,
    pub lint: LintView,
    pub catalog: SharedCatalog,
    pub problems: SharedProblems,
    pub squiggles: TextDecorationCollection,
    pub lint_task: Option<Task<()>>,
    pub save_task: Option<Task<()>>,
    pub saving: bool,
    pub _events: Subscription,
}

impl EditorPane {
    pub fn new(p: Palette, window: &mut Window, cx: &mut Context<ShadersView>) -> Self {
        let catalog: SharedCatalog = Rc::new(RefCell::new(Arc::new(Catalog::default())));
        let problems: SharedProblems = Rc::new(RefCell::new(Vec::new()));
        let state = cx.new(|cx| EditorState::new(window, cx).language("swfx"));
        state.update(cx, |s, cx| {
            s.set_highlighter_factory(
                Rc::new(move |lang| {
                    (lang == "swfx").then(|| Box::new(SwfxHighlighter::new(p)) as Box<dyn InputHighlighter>)
                }),
                cx,
            );
            let lsp = s.lsp_mut();
            lsp.completion_provider = Some(Rc::new(Completer {
                catalog: catalog.clone(),
            }));
            lsp.hover_provider = Some(Rc::new(Hoverer {
                catalog: catalog.clone(),
                problems: problems.clone(),
            }));
            lsp.code_action_providers.push(Rc::new(Fixer {
                problems: problems.clone(),
            }));
        });
        let squiggles = state.update(cx, |s, cx| s.create_decorations_collection(Vec::new(), cx));
        let events = cx.subscribe_in(&state, window, |this, _, ev: &InputEvent, _window, cx| {
            if matches!(ev, InputEvent::Change) {
                this.edited(cx);
            }
        });
        Self {
            state,
            loaded: None,
            loading: false,
            saved: String::new(),
            last_text: String::new(),
            lint: LintView::default(),
            catalog,
            problems,
            squiggles,
            lint_task: None,
            save_task: None,
            saving: false,
            _events: events,
        }
    }
}

struct SwfxHighlighter {
    palette: Palette,
    tokens: Vec<swofty_shaders::Token>,
}

impl SwfxHighlighter {
    fn new(palette: Palette) -> Self {
        Self {
            palette,
            tokens: Vec::new(),
        }
    }

    fn style(&self, kind: TokenKind) -> HighlightStyle {
        let p = self.palette;
        let (color, weight) = match kind {
            TokenKind::Comment => (p.text_placeholder, None),
            TokenKind::Header => (p.accent_hover, Some(FontWeight::SEMIBOLD)),
            TokenKind::Key => (p.text, None),
            TokenKind::Number => (p.warning, None),
            TokenKind::String => (p.success, None),
            TokenKind::Bool => (p.accent, None),
            TokenKind::Punct => (p.text_muted, None),
        };
        HighlightStyle {
            color: Some(color),
            font_weight: weight,
            ..Default::default()
        }
    }
}

impl InputHighlighter for SwfxHighlighter {
    fn language(&self) -> SharedString {
        "swfx".into()
    }

    fn update(
        &mut self,
        _edit: Option<InputEdit>,
        text: &Rope,
        _folding: bool,
        _window: &mut Window,
        _cx: &mut Context<EditorState>,
    ) {
        self.tokens = swofty_shaders::tokens(&text.to_string());
        self.tokens.sort_by_key(|t| t.range.start);
    }

    fn styles(
        &self,
        range: &Range<usize>,
        _resolver: &dyn HighlightStyleResolver,
    ) -> Vec<(Range<usize>, HighlightStyle)> {
        let mut out = Vec::new();
        let mut at = range.start;
        for token in &self.tokens {
            if token.range.end <= at {
                continue;
            }
            if token.range.start >= range.end {
                break;
            }
            let start = token.range.start.max(at);
            let end = token.range.end.min(range.end);
            if start > at {
                out.push((at..start, HighlightStyle::default()));
            }
            out.push((start..end, self.style(token.kind)));
            at = end;
        }
        if at < range.end {
            out.push((at..range.end, HighlightStyle::default()));
        }
        out
    }

    fn fold_ranges(&self, _text: &Rope) -> Vec<FoldRange> {
        Vec::new()
    }
}

fn lsp_position(text: &Rope, offset: usize) -> lsp_types::Position {
    text.offset_to_position(offset.min(text.len()))
}

struct Completer {
    catalog: SharedCatalog,
}

impl CompletionProvider for Completer {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        _trigger: CompletionContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<CompletionResponse>> {
        let catalog = self.catalog.borrow().clone();
        let source = text.to_string();
        let items = swofty_shaders::completions(&source, offset.min(source.len()), &catalog)
            .into_iter()
            .map(|c| CompletionItem {
                kind: Some(match c.kind {
                    CompletionKind::Effect => CompletionItemKind::CLASS,
                    CompletionKind::Param => CompletionItemKind::PROPERTY,
                    CompletionKind::Value => CompletionItemKind::VALUE,
                    CompletionKind::Keyword => CompletionItemKind::KEYWORD,
                }),
                detail: (!c.detail.is_empty()).then_some(c.detail),
                documentation: (!c.doc.is_empty()).then_some(Documentation::String(c.doc)),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: lsp_types::Range {
                        start: lsp_position(text, c.range.start),
                        end: lsp_position(text, c.range.end),
                    },
                    new_text: c.insert,
                })),
                label: c.label,
                ..Default::default()
            })
            .collect();
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut App) -> bool {
        new_text
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || matches!(c, '[' | '.' | '=' | ' ' | '"' | '_'))
    }
}

struct Hoverer {
    catalog: SharedCatalog,
    problems: SharedProblems,
}

impl HoverProvider for Hoverer {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Option<lsp_types::Hover>>> {
        let catalog = self.catalog.borrow().clone();
        let source = text.to_string();
        let docs = swofty_shaders::hover(&source, offset.min(source.len()), &catalog);
        let notes: Vec<String> = self
            .problems
            .borrow()
            .iter()
            .filter(|p| squiggle_range(&source, &p.range).is_some_and(|r| r.contains(&offset)))
            .map(|p| format!("**{}** {}", severity_label(p.severity), p.message))
            .collect();
        let range = docs.as_ref().map(|h| h.range.clone());
        let value = notes
            .into_iter()
            .chain(docs.map(|h| h.text))
            .collect::<Vec<_>>()
            .join("\n\n");
        let hover = (!value.is_empty()).then(|| lsp_types::Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: range.map(|r| lsp_types::Range {
                start: lsp_position(text, r.start),
                end: lsp_position(text, r.end),
            }),
        });
        Task::ready(Ok(hover))
    }
}

struct Fixer {
    problems: SharedProblems,
}

impl CodeActionProvider for Fixer {
    fn id(&self) -> SharedString {
        "swfx-fixes".into()
    }

    fn code_actions(
        &self,
        _state: Entity<EditorState>,
        range: Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Task<anyhow::Result<Vec<CodeAction>>> {
        let actions = self
            .problems
            .borrow()
            .iter()
            .filter(|p| p.range.start <= range.end && range.start <= p.range.end)
            .filter_map(|p| {
                let fix = p.fix.as_ref()?;
                Some(CodeAction {
                    title: fix.title.clone(),
                    kind: Some(CodeActionKind::QUICKFIX),
                    data: serde_json::to_value((fix.range.start, fix.range.end, &fix.replacement)).ok(),
                    ..Default::default()
                })
            })
            .collect();
        Task::ready(Ok(actions))
    }

    fn perform_code_action(
        &self,
        state: Entity<EditorState>,
        action: CodeAction,
        _push_to_history: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>> {
        if let Some((start, end, text)) = action
            .data
            .and_then(|d| serde_json::from_value::<(usize, usize, String)>(d).ok())
        {
            state.update(cx, |s, cx| apply_edit(s, start..end, &text, window, cx));
        }
        Task::ready(Ok(()))
    }
}

pub(super) fn apply_edit(
    state: &mut EditorState,
    range: Range<usize>,
    new_text: &str,
    window: &mut Window,
    cx: &mut Context<EditorState>,
) {
    let rope = state.text().clone();
    let len = rope.len();
    let edit = TextEdit {
        range: lsp_types::Range {
            start: lsp_position(&rope, range.start.min(len)),
            end: lsp_position(&rope, range.end.min(len)),
        },
        new_text: new_text.to_owned(),
    };
    state.apply_lsp_edits(&vec![edit], window, cx);
}

fn problem_of(d: swofty_shaders::Diagnostic) -> Problem {
    Problem {
        severity: d.severity,
        range: d.range,
        line: d.start.line,
        column: d.start.character,
        message: d.message,
        fix: d.fix,
    }
}

impl ShadersView {
    pub(super) fn edited(&mut self, cx: &mut Context<Self>) {
        let text = self.editor.state.read(cx).value().to_string();
        if text == self.editor.last_text {
            return;
        }
        self.editor.last_text = text.clone();
        self.schedule_lint(text, LINT_DEBOUNCE, cx);
    }

    pub(super) fn schedule_lint(&mut self, text: String, wait: Duration, cx: &mut Context<Self>) {
        self.editor.lint.rev += 1;
        let rev = self.editor.lint.rev;
        let catalog = self.catalog.clone();
        self.editor.lint_task = Some(cx.spawn(async move |this, cx| {
            if !wait.is_zero() {
                cx.background_executor().timer(wait).await;
            }
            let source = text.clone();
            let (problems, config, resolved) = cx
                .background_executor()
                .spawn(async move {
                    let lint = swofty_shaders::lint(&source, &catalog);
                    let instances = lint.config.as_ref().map(|c| swofty_shaders::resolve(c, &catalog));
                    let rules = lint
                        .config
                        .as_ref()
                        .map(|c| swofty_shaders::resolve_rules(c, &catalog));
                    let problems: Vec<Problem> = lint.diagnostics.into_iter().map(problem_of).collect();
                    (problems, lint.config, instances.zip(rules))
                })
                .await;
            this.update(cx, |v, cx| v.lint_done(rev, text, problems, config, resolved, cx))
                .ok();
        }));
    }

    fn lint_done(
        &mut self,
        rev: u64,
        text: String,
        problems: Vec<Problem>,
        config: Option<Config>,
        resolved: Option<(Vec<ActiveInstance>, Vec<swofty_shaders::Rule>)>,
        cx: &mut Context<Self>,
    ) {
        if rev != self.editor.lint.rev {
            return;
        }
        let p = palette(cx);
        let squiggles: Vec<TextDecoration> = problems
            .iter()
            .filter_map(|problem| {
                let range = squiggle_range(&text, &problem.range)?;
                let color = match problem.severity {
                    Severity::Error => p.error,
                    Severity::Warning => p.warning,
                    Severity::Info => p.text_muted,
                    Severity::Hint => p.text_placeholder,
                };
                Some(TextDecoration::new(
                    range,
                    HighlightStyle {
                        underline: Some(UnderlineStyle {
                            color: Some(color),
                            thickness: px(1.),
                            wavy: true,
                        }),
                        ..Default::default()
                    },
                ))
            })
            .fold(Vec::<TextDecoration>::new(), |mut out, d| {
                if !out
                    .iter()
                    .any(|o| o.range.start < d.range.end && d.range.start < o.range.end)
                {
                    out.push(d);
                }
                out
            });
        self.editor.squiggles.set(squiggles, cx);
        *self.editor.problems.borrow_mut() = problems.clone();
        let valid = config.is_some();
        self.editor.lint = LintView {
            rev,
            problems,
            done: true,
        };
        if valid {
            if let Some((instances, rules)) = resolved {
                self.preview_instances(instances, rules, cx);
            }
            self.schedule_save(text, cx);
        }
        cx.notify();
    }

    fn schedule_save(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(entry) = self.selected_entry().filter(|e| e.editable) else {
            return;
        };
        if text == self.editor.saved {
            return;
        }
        let rev = self.editor.lint.rev;
        let r = entry.r#ref.clone();
        let store = self.store.clone();
        let live = self.live_target(&r);
        self.editor.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            let stale = this.update(cx, |v, _| v.editor.lint.rev != rev).unwrap_or(true);
            if stale {
                return;
            }
            this.update(cx, |v, cx| {
                v.editor.saving = true;
                cx.notify();
            })
            .ok();
            let saved = text.clone();
            let done = cx
                .background_executor()
                .spawn(async move {
                    swofty_core::shaders::save(&store, &r, &text).map_err(|e| e.to_string())?;
                    if let Some((dir, name)) = live {
                        let profile = store.profile(&name).map_err(|e| e.to_string())?;
                        let active = swofty_core::shaders::active_file(&store, &profile);
                        swofty_core::shaders::push_live(&dir, &active).map_err(|e| e.to_string())?;
                    }
                    Ok::<(), String>(())
                })
                .await;
            this.update(cx, |v, cx| {
                v.editor.saving = false;
                match done {
                    Ok(()) => {
                        v.editor.saved = saved;
                        v.error = None;
                        v.refresh(cx);
                    }
                    Err(e) => v.error = Some(trf!("Couldn't save the config: {e}", e = e)),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    pub(super) fn jump_to(&mut self, line: u32, column: u32, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.editor.state.clone();
        state.update(cx, |s, cx| {
            s.set_cursor_position(Position::new(line, column), window, cx);
            s.focus(window, cx);
        });
    }

    pub(super) fn apply_fix(
        &mut self,
        fix: swofty_shaders::Fix,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = self.editor.state.clone();
        state.update(cx, |s, cx| {
            apply_edit(s, fix.range.clone(), &fix.replacement, window, cx)
        });
        self.edited(cx);
    }

    pub(super) fn editor_pane(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let entry = self.selected_entry().cloned();
        let Some(entry) = entry else {
            return self.empty_editor(p, cx);
        };
        let editable = entry.editable;
        let header = self.editor_header(&entry, p, cx);
        v_flex()
            .size_full()
            .min_h_0()
            .gap(px(10.))
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.surface)
                    .overflow_hidden()
                    .child(
                        Editor::new(&self.editor.state)
                            .bordered(false)
                            .h(relative(1.))
                            .readonly(!editable),
                    ),
            )
            .child(self.problems_strip(p, cx))
            .into_any_element()
    }

    fn editor_header(&self, entry: &ConfigEntry, p: Palette, cx: &mut Context<Self>) -> Div {
        let editable = entry.editable;
        let live = self.is_live(&entry.r#ref);
        let (pill_text, pill_color, pill_icon) = self.lint_status(live, p);
        let source = self.source_line(entry);
        let copy_id = entry.r#ref.clone();
        h_flex()
            .gap(px(10.))
            .items_center()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(entry.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(source),
                    ),
            )
            .when(!editable, |d| {
                d.child(
                    Button::new("shaders-copy-edit")
                        .outline()
                        .tap_sm()
                        .icon(IconName::Copy)
                        .label(tr("Copy to personal to edit"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.copy_to_personal(copy_id.clone(), window, cx)
                        })),
                )
            })
            .child(
                h_flex()
                    .h(px(24.))
                    .px(px(9.))
                    .gap(px(6.))
                    .items_center()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(pill_color.opacity(0.5))
                    .bg(pill_color.opacity(0.12))
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(pill_color)
                    .child(Icon::new(pill_icon).size(px(13.)))
                    .child(pill_text),
            )
    }

    fn lint_status(&self, live: bool, p: Palette) -> (String, Hsla, IconName) {
        let errors = self.editor.lint.errors();
        let warnings = self.editor.lint.warnings();
        if !self.editor.lint.done {
            (tr("Checking").to_owned(), p.text_muted, IconName::LoaderCircle)
        } else if errors > 0 {
            (
                trf!(
                    "{problems} · not applied",
                    problems = trn(errors, "{count} problem", "{count} problems")
                ),
                p.error,
                IconName::CircleAlert,
            )
        } else if live {
            (
                tr("Valid · live in game").to_owned(),
                p.success,
                IconName::CircleCheck,
            )
        } else if warnings > 0 {
            (
                trf!(
                    "Valid · {warnings}",
                    warnings = trn(warnings, "{count} warning", "{count} warnings")
                ),
                p.warning,
                IconName::TriangleAlert,
            )
        } else {
            (tr("Valid").to_owned(), p.success, IconName::CircleCheck)
        }
    }

    fn source_line(&self, entry: &ConfigEntry) -> String {
        if entry.editable {
            if self.editor.saving {
                tr("Saving")
            } else {
                tr("Saved automatically")
            }
            .to_owned()
        } else {
            trf!("From {source} · read-only", source = entry.source)
        }
    }

    fn empty_editor(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .size_full()
            .rounded(radius::md())
            .border_1()
            .border_dashed()
            .border_color(p.border)
            .child(
                EmptyState::new(IconName::Palette, tr("No config selected"))
                    .body(tr("Pick one on the right, or start a new one."))
                    .action(
                        Button::new("shaders-empty-new")
                            .outline()
                            .icon(IconName::Plus)
                            .label(tr("New config"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| this.new_config(window, cx))),
                    ),
            )
            .into_any_element()
    }

    fn problems_strip(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let problems = self.editor.lint.problems.clone();
        if problems.is_empty() {
            return div().into_any_element();
        }
        let counts = [
            (Severity::Error, p.error),
            (Severity::Warning, p.warning),
            (Severity::Info, p.accent),
        ]
        .into_iter()
        .filter_map(|(sev, color)| {
            let n = problems.iter().filter(|x| x.severity == sev).count();
            (n > 0).then(|| {
                h_flex()
                    .gap(px(4.))
                    .items_center()
                    .text_color(color)
                    .child(Icon::new(icon_of(sev)).size(px(13.)))
                    .child(div().text_size(px(12.)).child(n.to_string()))
            })
        })
        .collect::<Vec<_>>();
        let editable = self.selected_entry().is_some_and(|e| e.editable);
        let rows = problems.into_iter().enumerate().map(|(i, problem)| {
            let color = match problem.severity {
                Severity::Error => p.error,
                Severity::Warning => p.warning,
                _ => p.accent,
            };
            let (line, column) = (problem.line, problem.column);
            let fix = problem.fix.clone();
            h_flex()
                .id(SharedString::from(format!("problem-{i}")))
                .gap(px(8.))
                .px(px(10.))
                .py(px(5.))
                .items_center()
                .cursor_pointer()
                .hover(move |s| s.bg(p.hover))
                .on_click(cx.listener(move |this, _, window, cx| this.jump_to(line, column, window, cx)))
                .child(
                    Icon::new(icon_of(problem.severity))
                        .size(px(13.))
                        .text_color(color),
                )
                .child(
                    div()
                        .w(px(46.))
                        .flex_shrink_0()
                        .text_size(px(12.))
                        .text_color(p.text_placeholder)
                        .child(format!("{}:{}", problem.line + 1, problem.column + 1)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(13.))
                        .text_color(p.text)
                        .child(problem.message),
                )
                .when_some(fix.filter(|_| editable), |d, fix| {
                    d.child(
                        Button::new(SharedString::from(format!("problem-fix-{i}")))
                            .outline()
                            .tap_sm()
                            .label(fix.title.clone())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.apply_fix(fix.clone(), window, cx)
                            })),
                    )
                })
        });
        v_flex()
            .flex_shrink_0()
            .max_h(px(168.))
            .rounded(px(6.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                h_flex()
                    .px(px(10.))
                    .py(px(5.))
                    .gap(px(12.))
                    .items_center()
                    .border_b_1()
                    .border_color(p.border_variant)
                    .child(
                        div()
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.text_placeholder)
                            .child(tr("PROBLEMS")),
                    )
                    .children(counts),
            )
            .child(
                v_flex()
                    .id("shaders-problems")
                    .min_h_0()
                    .scroll_y("shaders-problems-scroll")
                    .children(rows),
            )
            .into_any_element()
    }

    pub(super) fn mode_switch(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let seg = |mode: Mode, label: &'static str, icon: IconName| {
            let on = self.mode == mode;
            h_flex()
                .id(SharedString::from(format!("shaders-mode-{label}")))
                .h(px(28.))
                .px(px(14.))
                .gap(px(7.))
                .items_center()
                .rounded(px(5.))
                .cursor_pointer()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .map(|d| {
                    if on {
                        d.bg(p.selected).text_color(p.text)
                    } else {
                        d.text_color(p.text_muted).hover(move |s| s.text_color(p.text))
                    }
                })
                .child(Icon::new(icon).size(px(14.)))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| this.set_mode(mode, cx)))
        };
        h_flex()
            .p(px(2.))
            .gap(px(2.))
            .rounded(px(7.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .child(seg(Mode::Editor, tr("Editor"), IconName::SquarePen))
            .child(seg(Mode::Preview, tr("Preview"), IconName::Play))
            .child(seg(Mode::Documentation, tr("Documentation"), IconName::BookOpen))
            .into_any_element()
    }
}

fn squiggle_range(text: &str, range: &Range<usize>) -> Option<Range<usize>> {
    let start = range.start.min(text.len());
    let end = range.end.min(text.len());
    if start < end {
        return Some(start..end);
    }
    if let Some(c) = text.get(start..).and_then(|t| t.chars().next()) {
        return Some(start..start + c.len_utf8());
    }
    let c = text.get(..start)?.chars().next_back()?;
    Some(start - c.len_utf8()..start)
}

fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Error => tr("Error"),
        Severity::Warning => tr("Warning"),
        Severity::Info => tr("Info"),
        Severity::Hint => tr("Hint"),
    }
}

fn icon_of(s: Severity) -> IconName {
    match s {
        Severity::Error => IconName::CircleAlert,
        Severity::Warning => IconName::TriangleAlert,
        Severity::Info | Severity::Hint => IconName::Info,
    }
}
