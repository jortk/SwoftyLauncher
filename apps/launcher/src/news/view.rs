use super::feed::{Post, format_date};
use super::markdown::{Block, SITE, Span, safe_url};
use super::state::{NewsPane, NewsRow, STACK_BELOW, StackedRow};
use crate::pages::GUTTER;
use crate::scale::{Hit, px};
use crate::shell::{Page, Shell};
use crate::theme::{Palette, ScrollArea};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Disableable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::{tr, trf};

pub(super) const TEXT: f32 = 15.;

pub(super) const LINE: f32 = 24.;

pub(super) fn news_tag(p: Palette) -> Div {
    div()
        .px(px(7.))
        .h(px(20.))
        .flex()
        .items_center()
        .rounded(px(4.))
        .border_1()
        .border_color(p.accent)
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.accent)
        .whitespace_nowrap()
        .child(tr("News"))
}

pub(super) fn rich(id: String, spans: &[Span], p: Palette, mono: &SharedString) -> AnyElement {
    let mut text = String::new();
    let mut highlights = Vec::new();
    let mut fonts = Vec::new();
    let mut ranges = Vec::new();
    let mut urls = Vec::new();
    for span in spans {
        let start = text.len();
        text.push_str(&span.text);
        let range = start..text.len();
        let mut style = HighlightStyle::default();
        if span.strong {
            style.font_weight = Some(FontWeight::BOLD);
            style.color = Some(p.text);
        }
        if span.emphasis {
            style.font_style = Some(FontStyle::Italic);
        }
        if span.code {
            style.color = Some(p.text);
            style.background_color = Some(p.elevated);
            fonts.push((range.clone(), mono.clone()));
        }
        if let Some(url) = &span.link {
            style.color = Some(p.accent_hover);
            style.underline = Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(p.accent_hover.opacity(0.45)),
                wavy: false,
            });
            ranges.push(range.clone());
            urls.push(url.clone());
        }
        if style != HighlightStyle::default() {
            highlights.push((range, style));
        }
    }
    let styled = StyledText::new(text)
        .with_highlights(highlights)
        .with_font_family_overrides(fonts);
    if ranges.is_empty() {
        return styled.into_any_element();
    }
    InteractiveText::new(SharedString::from(id), styled)
        .on_click(ranges, move |ix, _, cx| {
            if let Some(url) = urls.get(ix) {
                cx.open_url(url);
            }
        })
        .into_any_element()
}

pub(super) fn parsed_body(post: &Post) -> Vec<Block> {
    if post.body.trim().is_empty() {
        vec![Block::Paragraph(super::markdown::inline(&post.summary))]
    } else {
        super::markdown::parse(&post.body)
    }
}

pub(crate) fn render_markdown(id: &str, text: &str, p: Palette, mono: &SharedString) -> Vec<AnyElement> {
    blocks_view(id, &super::markdown::parse(text), p, mono)
}

pub(super) fn blocks_view(
    prefix: &str,
    blocks: &[Block],
    p: Palette,
    mono: &SharedString,
) -> Vec<AnyElement> {
    blocks
        .iter()
        .enumerate()
        .map(|(i, block)| block_view(prefix, i, block, i.checked_sub(1).map(|j| &blocks[j]), p, mono))
        .collect()
}

pub(super) fn block_view(
    prefix: &str,
    i: usize,
    block: &Block,
    previous: Option<&Block>,
    p: Palette,
    mono: &SharedString,
) -> AnyElement {
    let text = p.text.opacity(0.84);
    let id = format!("{prefix}-{i}");
    let top = match (previous, block) {
        (None, _) => 0.,
        (_, Block::Heading(..)) => 26.,
        (Some(Block::Heading(..)), _) => 8.,
        (Some(Block::Item { .. }), Block::Item { .. }) => 8.,
        (_, Block::Rule) | (Some(Block::Rule), _) => 22.,
        _ => 12.,
    };
    let el = match block {
        Block::Heading(level, spans) => div()
            .text_size(px(match level {
                1 => 21.,
                2 => 18.,
                _ => 15.5,
            }))
            .line_height(px(if *level <= 2 { 26. } else { 22. }))
            .font_weight(FontWeight::BOLD)
            .text_color(p.text)
            .child(rich(id, spans, p, mono))
            .into_any_element(),
        Block::Paragraph(spans) => div()
            .text_color(text)
            .child(rich(id, spans, p, mono))
            .into_any_element(),
        Block::Item { depth, number, spans } => h_flex()
            .items_start()
            .gap(px(12.))
            .pl(px(2. + 22. * *depth as f32))
            .child(match number {
                Some(n) => div()
                    .min_w(px(18.))
                    .flex_shrink_0()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.accent_hover)
                    .child(format!("{n}.")),
                None => div()
                    .mt(px((LINE - 6.) / 2.))
                    .size(px(6.))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(if *depth == 0 { p.accent } else { p.text_placeholder }),
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(text)
                    .child(rich(id, spans, p, mono)),
            )
            .into_any_element(),
        Block::Quote(spans) => div()
            .pl(px(14.))
            .border_l_2()
            .border_color(p.accent)
            .text_color(p.text_muted)
            .child(rich(id, spans, p, mono))
            .into_any_element(),
        Block::Code(code) => div()
            .px(px(14.))
            .py(px(10.))
            .rounded(px(6.))
            .bg(p.chrome)
            .border_1()
            .border_color(p.border_variant)
            .font_family(mono.clone())
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(p.text)
            .child(code.clone())
            .into_any_element(),
        Block::Rule => div().h(px(1.)).bg(p.border_variant).into_any_element(),
    };
    div().pt(px(top)).child(el).into_any_element()
}

pub(super) fn changelog_header(count: usize, p: Palette) -> Div {
    h_flex()
        .flex_shrink_0()
        .px(px(16.))
        .h(px(46.))
        .gap(px(8.))
        .items_center()
        .border_b_1()
        .border_color(p.border)
        .child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(tr("Changelogs")),
        )
        .child(div().flex_1())
        .child(
            div()
                .text_size(px(12.))
                .text_color(p.text_placeholder)
                .child(trf!("{count} releases", count = count)),
        )
}

pub(super) fn changelog_row(post: &Post, first: bool, p: Palette) -> AnyElement {
    let url = post.url.as_deref().and_then(safe_url);
    let current = post.version() == env!("CARGO_PKG_VERSION");
    v_flex()
        .id(SharedString::from(format!("news-{}", post.id)))
        .flex_shrink_0()
        .px(px(16.))
        .py(px(11.))
        .gap(px(4.))
        .when(!first, |d| d.border_t_1().border_color(p.border_variant))
        .when_some(url, |d, url| {
            d.cursor_pointer()
                .hover(move |s| s.bg(p.hover))
                .on_click(move |_, _, cx| cx.open_url(&url))
        })
        .child(
            h_flex()
                .gap(px(8.))
                .items_center()
                .child(
                    div()
                        .flex_shrink_0()
                        .whitespace_nowrap()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(post.version().to_owned()),
                )
                .when(current, |d| {
                    d.child(
                        div()
                            .flex_shrink_0()
                            .px(px(6.))
                            .h(px(18.))
                            .flex()
                            .items_center()
                            .rounded(px(4.))
                            .bg(p.accent.opacity(0.18))
                            .text_size(px(11.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.accent_hover)
                            .child(tr("Installed")),
                    )
                })
                .child(div().flex_1())
                .child(
                    div()
                        .flex_shrink_0()
                        .whitespace_nowrap()
                        .text_size(px(12.))
                        .text_color(p.text_placeholder)
                        .child(format_date(&post.date)),
                ),
        )
        .when(!post.summary.is_empty(), |d| {
            d.child(
                div()
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .text_color(p.text_muted)
                    .line_clamp(2)
                    .text_ellipsis()
                    .child(post.summary.clone()),
            )
        })
        .into_any_element()
}

pub(super) fn empty(text: &'static str, p: Palette) -> Div {
    div()
        .py(px(4.))
        .text_size(px(13.))
        .text_color(p.text_placeholder)
        .child(text)
}

impl Shell {
    pub(crate) fn is_card_dismissed(&self, id: &str) -> bool {
        self.store.config().dismissed_cards.contains(id)
    }

    pub(crate) fn dismiss_card(&mut self, id: &str, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        if config.dismissed_cards.insert(id.to_string()) {
            self.store.save_config(&config).ok();
            cx.notify();
        }
    }

    pub(crate) fn news_page(&mut self, p: Palette, short: bool, cx: &mut Context<Self>) -> AnyElement {
        if self.news.stale() {
            self.fetch_news(cx);
        }
        let refresh = Button::new("news-refresh")
            .ghost()
            .tap_sm()
            .icon(IconName::RefreshCw)
            .label(tr("Refresh"))
            .disabled(self.news.fetching)
            .on_click(cx.listener(|this, _, _, cx| this.fetch_news(cx)));
        let stacked = self.content_width < STACK_BELOW;
        let side = (self.content_width * 0.32).clamp(260., 380.);
        let side = self.ad_rail_width().map_or(side, |w| side.max(w));
        let show_banner = !self.is_card_dismissed(crate::discord::BANNER_CARD_ID);
        self.news_scroll.sync(&self.news, short, stacked, show_banner);
        if stacked {
            return v_flex()
                .size_full()
                .child(self.header(Page::News.label(), self.news.subtitle(), refresh, p))
                .child(self.stacked_news(short, cx))
                .into_any_element();
        }
        let posts = self.news_posts(p, cx);
        let changelogs = self.changelogs(p, cx);
        let columns = h_flex()
            .flex_1()
            .min_h_0()
            .items_stretch()
            .gap(px(16.))
            .child(posts)
            .child(
                v_flex()
                    .w(px(side))
                    .flex_shrink_0()
                    .min_h_0()
                    .gap(px(16.))
                    .child(changelogs)
                    .children(self.ad_rail(p)),
            );
        let content = v_flex()
            .flex_1()
            .min_h_0()
            .px(px(GUTTER))
            .pt(px(4.))
            .pb(px(20.))
            .gap(px(16.))
            .when(show_banner, |d| {
                d.child(crate::discord::banner(
                    short,
                    cx.listener(|this, _, _, cx| {
                        this.dismiss_card(crate::discord::BANNER_CARD_ID, cx);
                    }),
                ))
            })
            .child(columns);
        v_flex()
            .size_full()
            .child(self.header(Page::News.label(), self.news.subtitle(), refresh, p))
            .map(|d| {
                if stacked {
                    d.child(content.scroll_y("news"))
                } else {
                    d.child(content)
                }
            })
            .into_any_element()
    }

    pub(super) fn news_scrollbar(&self, pane: NewsPane, cx: &Context<Self>) -> Div {
        self.news_scroll.motion[pane as usize].overlay(self.news_scroll.list(pane), cx)
    }

    pub(super) fn news_posts(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let frame = v_flex()
            .relative()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden();
        if self.news_scroll.rows.is_empty() {
            return frame
                .flex_1()
                .child(empty(tr("No news yet."), p).p(px(32.)))
                .into_any_element();
        }
        let scroll = &self.news_scroll.articles;
        frame
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(
                list(
                    scroll.clone(),
                    cx.processor(|this, i, _, cx| {
                        let p = crate::theme::palette(cx);
                        let mono = cx.theme().mono_font_family.clone();
                        div()
                            .w_full()
                            .px(px(32.))
                            // Include edge spacing in row measurements so virtualization
                            // keeps partially visible rows until they leave the viewport.
                            .when(i == 0, |d| d.pt(px(28.)))
                            .when(i + 1 == this.news_scroll.rows.len(), |d| d.pb(px(36.)))
                            .child(this.news_row(i, p, &mono))
                            .into_any_element()
                    }),
                )
                .flex_1()
                .min_h_0(),
            )
            .child(self.news_scrollbar(NewsPane::Articles, cx))
            .into_any_element()
    }

    pub(super) fn news_row(&self, i: usize, p: Palette, mono: &SharedString) -> AnyElement {
        let Some(row) = self.news_scroll.rows.get(i) else {
            return div().into_any_element();
        };
        let content = match *row {
            NewsRow::Header(pi, first) => self.article_header(&self.news.posts[pi], first, p),
            NewsRow::Block(pi, bi) => {
                let blocks = &self.news.blocks[pi];
                div()
                    .text_size(px(TEXT))
                    .line_height(px(LINE))
                    .when(bi == 0, |d| d.pt(px(14.)))
                    .child(block_view(
                        &format!("news-{}", self.news.posts[pi].id),
                        bi,
                        &blocks[bi],
                        bi.checked_sub(1).map(|j| &blocks[j]),
                        p,
                        mono,
                    ))
                    .into_any_element()
            }
            NewsRow::Link(pi) => self
                .article_link(&self.news.posts[pi])
                .unwrap_or_else(|| div().into_any_element()),
            NewsRow::Separator => {
                return div()
                    .py(px(34.))
                    .child(div().h(px(1.)).bg(p.border))
                    .into_any_element();
            }
        };
        div().w_full().max_w(px(780.)).child(content).into_any_element()
    }

    pub(super) fn article_header(&self, post: &Post, first: bool, p: Palette) -> AnyElement {
        let image = post.image.as_ref().and_then(|u| self.news.images.get(u)).cloned();
        v_flex()
            .flex_shrink_0()
            .w_full()
            .max_w(px(780.))
            .children(image.map(|i| {
                div()
                    .w_full()
                    .h(px(220.))
                    .mb(px(22.))
                    .rounded(px(8.))
                    .overflow_hidden()
                    .child(img(i).size_full().object_fit(ObjectFit::Cover))
            }))
            .child(
                h_flex().gap(px(10.)).items_center().child(news_tag(p)).child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.text_placeholder)
                        .child(format_date(&post.date)),
                ),
            )
            .child(
                div()
                    .pt(px(10.))
                    .text_size(px(if first { 26. } else { 22. }))
                    .line_height(px(if first { 32. } else { 28. }))
                    .font_weight(FontWeight::BOLD)
                    .child(post.title.clone()),
            )
            .into_any_element()
    }

    pub(super) fn article_link(&self, post: &Post) -> Option<AnyElement> {
        let url = post.url.as_deref().and_then(safe_url)?;
        let on_site = url.starts_with(SITE);
        Some(
            h_flex()
                .pt(px(24.))
                .child(
                    Button::new(SharedString::from(format!("news-open-{}", post.id)))
                        .outline()
                        .tap_sm()
                        .icon(IconName::ExternalLink)
                        .label(if on_site {
                            tr("Read on f1.swofty.net")
                        } else {
                            tr("Open in browser")
                        })
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                )
                .into_any_element(),
        )
    }

    pub(super) fn changelogs(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let scroll = &self.news_scroll.releases;
        let count = self.news_scroll.release_indices.len();
        v_flex()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .flex_1()
            .min_h_0()
            .child(changelog_header(count, p))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .when(count == 0, |d| {
                        d.child(empty(tr("No release notes yet."), p).px(px(16.)).py(px(12.)))
                    })
                    .child(
                        list(
                            scroll.clone(),
                            cx.processor(|this, i, _, cx| {
                                let p = crate::theme::palette(cx);
                                let row = this
                                    .news_scroll
                                    .release_indices
                                    .get(i)
                                    .map(|&pi| changelog_row(&this.news.posts[pi], i == 0, p));
                                div().w_full().children(row).into_any_element()
                            }),
                        )
                        .flex_1()
                        .min_h_0(),
                    )
                    .child(self.news_scrollbar(NewsPane::Releases, cx)),
            )
            .into_any_element()
    }

    pub(super) fn stacked_news(&self, short: bool, cx: &mut Context<Self>) -> AnyElement {
        let scroll = &self.news_scroll.stacked;
        div()
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(
                list(
                    scroll.clone(),
                    cx.processor(move |this, i, _, cx| {
                        let p = crate::theme::palette(cx);
                        let mono = cx.theme().mono_font_family.clone();
                        let news = this.news_scroll.rows.len();
                        let releases = this.news_scroll.release_indices.len();
                        let panel = || div().bg(p.surface).border_x_1().border_color(p.border);
                        let row = match this.news_scroll.stacked_rows[i] {
                            StackedRow::Banner => div()
                                .pb(px(16.))
                                .child(crate::discord::banner(
                                    short,
                                    cx.listener(|this, _, _, cx| {
                                        this.dismiss_card(crate::discord::BANNER_CARD_ID, cx);
                                    }),
                                ))
                                .into_any_element(),
                            StackedRow::ArticlesTop => panel()
                                .rounded_t(px(10.))
                                .border_t_1()
                                .pt(px(28.))
                                .when(news == 0, |d| d.child(empty(tr("No news yet."), p).px(px(32.))))
                                .into_any_element(),
                            StackedRow::Article(row) => panel()
                                .px(px(32.))
                                .child(this.news_row(row, p, &mono))
                                .into_any_element(),
                            StackedRow::ArticlesBottom => panel()
                                .rounded_b(px(10.))
                                .border_b_1()
                                .h(px(36.))
                                .mb(px(16.))
                                .into_any_element(),
                            StackedRow::ReleasesHeader => panel()
                                .rounded_t(px(10.))
                                .border_t_1()
                                .child(changelog_header(releases, p))
                                .when(releases == 0, |d| {
                                    d.child(empty(tr("No release notes yet."), p).px(px(16.)).py(px(12.)))
                                })
                                .into_any_element(),
                            StackedRow::Release(ri) => {
                                let pi = this.news_scroll.release_indices[ri];
                                panel()
                                    .child(changelog_row(&this.news.posts[pi], ri == 0, p))
                                    .into_any_element()
                            }
                            StackedRow::ReleasesBottom => panel()
                                .rounded_b(px(10.))
                                .border_b_1()
                                .h(px(1.))
                                .into_any_element(),
                        };
                        div()
                            .w_full()
                            .px(px(GUTTER))
                            .when(i == 0, |d| d.pt(px(4.)))
                            .when(i + 1 == this.news_scroll.stacked_rows.len(), |d| d.pb(px(20.)))
                            .child(row)
                            .into_any_element()
                    }),
                )
                .flex_1()
                .min_h_0(),
            )
            .child(self.news_scrollbar(NewsPane::Stacked, cx))
            .into_any_element()
    }
}
