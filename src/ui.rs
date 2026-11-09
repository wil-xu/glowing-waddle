use std::ops::Range;

use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::BorderType;
use ratatui::widgets::Padding;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::app::App;
use crate::app::CommentPath;
use crate::app::Feed;
use crate::app::Hits;
use crate::app::Mode;
use crate::app::Query;
use crate::app::Sort;
use crate::app::Target;
use crate::data::Comment;
use crate::data::Post;
use crate::data::Vote;

const ORANGE: Color = Color::Rgb(255, 69, 0);
const PERIWINKLE: Color = Color::Rgb(113, 147, 255);
const DIM: Color = Color::Rgb(129, 131, 132);
const BORDER: Color = Color::Rgb(52, 53, 54);
const HIGHLIGHT_BG: Color = Color::Rgb(39, 39, 41);

const NAV_WIDTH: u16 = 22;
const SIDEBAR_WIDTH: u16 = 32;
const VOTE_WIDTH: u16 = 6;
const PREVIEW_LINES: usize = 2;

fn compact(n: i64) -> String {
    if n.abs() < 1_000 {
        return n.to_string();
    }
    let k = format!("{:.1}", n as f64 / 1_000.0);
    format!("{}k", k.trim_end_matches(".0"))
}

fn age(hours: u32) -> String {
    match hours {
        0 => "just now".to_owned(),
        1..24 => format!("{hours}h ago"),
        _ => format!("{}d ago", hours / 24),
    }
}

/// Greedy word wrap. Words longer than `width` are hard-split.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    text.split('\n')
        .flat_map(|para| {
            let mut lines = Vec::new();
            let mut cur = String::new();
            for word in para.split_whitespace() {
                let mut word: Vec<char> = word.chars().collect();
                while word.len() > width {
                    if !cur.is_empty() {
                        lines.push(std::mem::take(&mut cur));
                    }
                    lines.push(word.drain(..width).collect());
                }
                let word_len = word.len();
                let word: String = word.into_iter().collect();
                let needed = cur.chars().count() + usize::from(!cur.is_empty()) + word_len;
                if needed > width && !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                }
                if !cur.is_empty() {
                    cur.push(' ');
                }
                cur.push_str(&word);
            }
            lines.push(cur);
            lines
        })
        .collect()
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let mut s: String = text.chars().take(width.saturating_sub(1)).collect();
    s.push('…');
    s
}

fn vote_color(vote: Vote) -> Color {
    match vote {
        Vote::Up => ORANGE,
        Vote::None => Color::White,
        Vote::Down => PERIWINKLE,
    }
}

/// Draws the whole screen and records in `app.hits` where each clickable
/// thing ended up.
pub(crate) fn render(frame: &mut Frame, app: &mut App) {
    let mut hits = Hits::default();
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    render_header(frame, app, header, &mut hits);
    render_footer(frame, app, footer);

    let show_nav = body.width >= 70;
    let show_sidebar = body.width >= 100;
    let [nav, main, sidebar] = Layout::horizontal([
        Constraint::Length(if show_nav { NAV_WIDTH } else { 0 }),
        Constraint::Min(0),
        Constraint::Length(if show_sidebar { SIDEBAR_WIDTH } else { 0 }),
    ])
    .areas(body);

    if show_nav {
        render_nav(frame, app, nav, &mut hits);
    }
    if show_sidebar {
        render_sidebar(frame, app, sidebar);
    }
    match app.mode {
        Mode::Post { scroll } => render_post(frame, app, main, scroll, &mut hits),
        Mode::Feed | Mode::Search => render_feed(frame, app, main, &mut hits),
    }
    app.hits = Some(hits);
}

fn render_header(frame: &mut Frame, app: &App, area: Rect, hits: &mut Hits) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(BORDER);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [logo, search, user] = Layout::horizontal([
        Constraint::Length(12),
        Constraint::Min(10),
        Constraint::Length(28),
    ])
    .spacing(2)
    .areas(inner);

    frame.render_widget(
        Line::from(vec![
            Span::styled(" ◉ ", Style::new().fg(Color::White).bg(ORANGE).bold()),
            Span::styled(" workit", Style::new().fg(Color::White).bold()),
        ]),
        logo,
    );

    let searching = app.mode == Mode::Search;
    let search_line = if searching {
        Line::from(vec![
            Span::styled(" ⌕ ", Style::new().fg(ORANGE)),
            Span::raw(app.query.clone()),
            Span::styled(
                "▏",
                Style::new().fg(ORANGE).add_modifier(Modifier::SLOW_BLINK),
            ),
        ])
    } else if app.query.is_empty() {
        Line::from(vec![
            Span::styled(" ⌕ ", Style::new().fg(DIM)),
            Span::styled(format!("Search {}", app.feed_name()), Style::new().fg(DIM)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" ⌕ ", Style::new().fg(DIM)),
            Span::raw(app.query.clone()),
            Span::styled("  (esc to clear)", Style::new().fg(DIM)),
        ])
    };
    let bg = if searching {
        HIGHLIGHT_BG
    } else {
        Color::Reset
    };
    frame.render_widget(Paragraph::new(search_line).bg(bg), search);
    hits.regions.push((search, Target::Search));

    frame.render_widget(
        Line::from(vec![
            Span::styled("● ", Style::new().fg(Color::Green)),
            Span::styled("u/you", Style::new().bold()),
            Span::styled(
                format!("  ✦ {} karma", compact(i64::from(app.karma))),
                Style::new().fg(DIM),
            ),
        ])
        .right_aligned(),
        user,
    );
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    if let Some(status) = app.status {
        frame.render_widget(
            Line::from(Span::styled(
                format!(" ✔ {status} "),
                Style::new().fg(Color::Black).bg(Color::Green),
            )),
            area,
        );
        return;
    }

    let hints: &[(&str, &str)] = match app.mode {
        Mode::Feed if !app.query.is_empty() => &[
            ("j/k", "move"),
            ("enter", "open"),
            ("/", "edit search"),
            ("esc", "clear search"),
            ("q", "quit"),
        ],
        // Feed hints must fit on one line at 80 columns.
        Mode::Feed if matches!(app.feed, Feed::Group(_)) => &[
            ("a/z", "vote"),
            ("enter", "open"),
            ("1-4", "sort"),
            ("[/]", "feed"),
            ("/", "search"),
            ("s", "save"),
            ("J", "join"),
            ("q", "quit"),
        ],
        Mode::Feed => &[
            ("a/z", "vote"),
            ("enter", "open"),
            ("1-4", "sort"),
            ("[/]", "feed"),
            ("/", "search"),
            ("s", "save"),
            ("q", "quit"),
        ],
        Mode::Search => &[
            ("type", "filter"),
            ("w/ u/ flair:", "narrow"),
            ("enter", "keep"),
            ("esc", "clear"),
        ],
        Mode::Post { .. } if app.thread.focus.is_some() => &[
            ("j/k", "scroll"),
            ("n/p", "comments"),
            ("a/z", "vote comment"),
            ("enter", "collapse"),
            ("esc", "unfocus"),
        ],
        Mode::Post { .. } => &[
            ("j/k", "scroll"),
            ("n/p", "comments"),
            ("a/z", "vote"),
            ("s", "save"),
            ("c", "share"),
            ("esc", "back"),
        ],
    };
    let spans: Vec<Span> = hints
        .iter()
        .flat_map(|(key, what)| {
            [
                Span::styled(format!(" {key} "), Style::new().fg(ORANGE).bold()),
                Span::styled(format!("{what} "), Style::new().fg(DIM)),
            ]
        })
        .collect();
    frame.render_widget(Line::from(spans), area);
}

fn render_nav(frame: &mut Frame, app: &App, area: Rect, hits: &mut Hits) {
    let item = |label: String, feed: Feed| {
        let style = if app.feed == feed {
            Style::new().fg(ORANGE).bg(HIGHLIGHT_BG).bold()
        } else {
            Style::new()
        };
        Line::from(Span::styled(
            format!(" {:<w$}", label, w = usize::from(NAV_WIDTH) - 3),
            style,
        ))
    };

    // Each row, with the feed a click on it switches to.
    let rows: Vec<(Line, Option<Feed>)> = [
        (item("⌂ Home".to_owned(), Feed::Home), Some(Feed::Home)),
        (
            item("↗ Popular".to_owned(), Feed::Popular),
            Some(Feed::Popular),
        ),
        (item("◎ All".to_owned(), Feed::All), Some(Feed::All)),
        (item("☆ Saved".to_owned(), Feed::Saved), Some(Feed::Saved)),
        (Line::raw(""), None),
        (Line::styled(" GROUPS", Style::new().fg(DIM)), None),
    ]
    .into_iter()
    .chain(app.groups.iter().enumerate().map(|(i, g)| {
        let mark = if g.joined { "●" } else { "○" };
        let label = truncate(&format!("{mark} w/{}", g.name), usize::from(NAV_WIDTH) - 3);
        (item(label, Feed::Group(i)), Some(Feed::Group(i)))
    }))
    .collect();

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(BORDER);
    let inner = block.inner(area);
    let (lines, feeds): (Vec<Line>, Vec<Option<Feed>>) = rows.into_iter().unzip();
    frame.render_widget(Paragraph::new(lines).block(block), area);

    for (y, feed) in (inner.y..inner.bottom()).zip(feeds) {
        if let Some(feed) = feed {
            let row = Rect {
                y,
                height: 1,
                ..inner
            };
            hits.regions.push((row, Target::Feed(feed)));
        }
    }
}

fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let width = usize::from(area.width.saturating_sub(4));
    let section = |title: &str| {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(BORDER)
            .title(Span::styled(format!(" {title} "), Style::new().bold()))
            .padding(Padding::horizontal(1))
    };

    let mut about: Vec<Line> = match app.feed {
        Feed::Group(g) => {
            let group = &app.groups[g];
            let mut lines: Vec<Line> = wrap(group.description, width)
                .into_iter()
                .map(Line::raw)
                .collect();
            lines.extend([
                Line::raw(""),
                Line::from(vec![
                    Span::styled(compact(i64::from(group.members)), Style::new().bold()),
                    Span::styled(" members  ", Style::new().fg(DIM)),
                    Span::styled("● ", Style::new().fg(Color::Green)),
                    Span::styled(compact(i64::from(group.online)), Style::new().bold()),
                    Span::styled(" online", Style::new().fg(DIM)),
                ]),
            ]);
            lines
        }
        Feed::Home | Feed::Popular | Feed::All | Feed::Saved => wrap(
            "Your personal workit frontpage. Come here to check in with your favorite groups.",
            width,
        )
        .into_iter()
        .map(Line::raw)
        .collect(),
    };
    let button = match app.feed {
        Feed::Group(g) if app.groups[g].joined => " Joined ",
        Feed::Group(_) => " Join ",
        Feed::Home | Feed::Popular | Feed::All | Feed::Saved => " Create Post ",
    };
    about.push(Line::raw(""));
    about.push(
        Line::from(Span::styled(
            button,
            Style::new().fg(Color::White).bg(ORANGE).bold(),
        ))
        .centered(),
    );
    let about_title = match app.feed {
        Feed::Group(g) => format!("About w/{}", app.groups[g].name),
        Feed::Home | Feed::Popular | Feed::All | Feed::Saved => app.feed_name(),
    };

    let mut trending: Vec<&Post> = app.posts.iter().collect();
    trending.sort_by_key(|p| std::cmp::Reverse(p.comment_count));
    let trending: Vec<Line> = trending
        .iter()
        .take(4)
        .flat_map(|p| {
            [
                Line::styled(truncate(p.title, width), Style::new().bold()),
                Line::styled(
                    truncate(
                        &format!(
                            "w/{} · {} comments",
                            app.groups[p.group].name,
                            compact(i64::from(p.comment_count))
                        ),
                        width,
                    ),
                    Style::new().fg(DIM),
                ),
            ]
        })
        .collect();

    let mut popular: Vec<_> = app.groups.iter().collect();
    popular.sort_by_key(|g| std::cmp::Reverse(g.members));
    let popular: Vec<Line> = popular
        .iter()
        .take(5)
        .enumerate()
        .map(|(rank, g)| {
            Line::from(vec![
                Span::styled(format!("{}  ", rank + 1), Style::new().fg(DIM)),
                Span::raw(format!("w/{}", g.name)),
                Span::styled(
                    format!("  {}", compact(i64::from(g.members))),
                    Style::new().fg(DIM),
                ),
            ])
        })
        .collect();

    // Boxes are stacked in priority order. A box that doesn't fit whole is
    // skipped rather than squeezed, except About, which is always drawn.
    let boxes = [
        (about_title, about),
        ("Trending today".to_owned(), trending),
        ("Popular groups".to_owned(), popular),
    ];
    let mut y = area.y;
    for (i, (title, lines)) in boxes.into_iter().enumerate() {
        let height = lines.len() as u16 + 2;
        let remaining = area.bottom().saturating_sub(y);
        if i > 0 && height > remaining {
            continue;
        }
        let rect = Rect {
            y,
            height: height.min(remaining),
            ..area
        };
        frame.render_widget(Paragraph::new(lines).block(section(&title)), rect);
        y += rect.height;
    }
}

fn render_feed(frame: &mut Frame, app: &mut App, area: Rect, hits: &mut Hits) {
    let [tabs, cards] = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

    let tab_spans: Vec<Span> = Sort::ALL
        .iter()
        .enumerate()
        .flat_map(|(i, &sort)| {
            let style = if sort == app.sort {
                Style::new()
                    .fg(ORANGE)
                    .bold()
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                Style::new().fg(DIM)
            };
            [
                Span::styled(format!(" {} ", i + 1), Style::new().fg(BORDER)),
                Span::styled(sort.label(), style),
                Span::raw("  "),
            ]
        })
        .collect();
    let tab_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(BORDER)
        .title(Span::styled(
            format!(" {} ", app.feed_name()),
            Style::new().fg(Color::White).bold(),
        ))
        .title_bottom(
            Line::styled(format!(" {} posts ", app.view.len()), Style::new().fg(DIM))
                .right_aligned(),
        );
    let tab_row = tab_block.inner(tabs);
    // Each tab is 3 spans: " N ", the label and a gap. The number and the
    // label are clickable, as far as they are drawn.
    let mut x = tab_row.x;
    for (&sort, spans) in Sort::ALL.iter().zip(tab_spans.chunks(3)) {
        let [number, label, gap] = [0, 1, 2].map(|i| spans[i].width() as u16);
        let rect = Rect::new(x + 1, tab_row.y, number - 1 + label, 1).intersection(tab_row);
        if !rect.is_empty() {
            hits.regions.push((rect, Target::Sort(sort)));
        }
        x += number + label + gap;
    }
    frame.render_widget(Paragraph::new(Line::from(tab_spans)).block(tab_block), tabs);

    if app.view.is_empty() {
        // A query that filters nothing (`w/`, `" "`) didn't empty the feed.
        // This reads `query`, which the header shows, rather than trusting
        // that `search` was refreshed; it only runs for an empty feed.
        let (headline, hint) = if !Query::parse(&app.query).is_empty() {
            (
                "Hm... we couldn't find any results",
                "Double-check your spelling, or press esc to clear the search",
            )
        } else if app.feed == Feed::Saved && !app.posts.iter().any(|p| p.saved) {
            ("No saved posts yet", "Press s on any post to save it")
        } else {
            (
                "Nothing here yet",
                "Try another sort with 1-4, or another feed with [ and ]",
            )
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::raw(""),
                Line::styled(headline, Style::new().bold()),
                Line::styled(hint, Style::new().fg(DIM)),
            ])
            .centered()
            .wrap(Wrap { trim: true }),
            cards,
        );
        return;
    }

    let content_width = usize::from(cards.width.saturating_sub(VOTE_WIDTH + 4));
    let heights: Vec<u16> = app
        .view
        .iter()
        .map(|&i| card_height(&app.posts[i], content_width))
        .collect();

    // Scroll just far enough that the selected card is fully visible.
    app.offset = app.offset.min(app.selected);
    while app.offset < app.selected
        && heights[app.offset..=app.selected].iter().sum::<u16>() > cards.height
    {
        app.offset += 1;
    }

    let mut y = cards.y;
    for (vi, &pi) in app.view.iter().enumerate().skip(app.offset) {
        let bottom = cards.y + cards.height;
        if y >= bottom {
            break;
        }
        let rect = Rect::new(cards.x, y, cards.width, heights[vi].min(bottom - y));
        hits.regions.push((rect, Target::Card(vi)));
        render_card(frame, app, vi, &app.posts[pi], rect, hits);
        y += heights[vi];
    }
}

/// `wrap`'s rows, each made entirely of text.
fn whole(rows: Vec<String>) -> Vec<(String, usize)> {
    rows.into_iter()
        .map(|row| {
            let len = row.chars().count();
            (row, len)
        })
        .collect()
}

/// The body with newlines flattened, wrapped, and cut to two rows with `…`.
/// Each row comes with how many of its leading chars are body text: all of
/// them, except in a cut row, which ends with the `…`.
fn preview(post: &Post, width: usize) -> Vec<(String, usize)> {
    let flat = post.body.replace('\n', " ");
    let mut lines = whole(wrap(&flat, width));
    if lines.len() > PREVIEW_LINES {
        lines.truncate(PREVIEW_LINES);
        let (last, len) = lines.last_mut().expect("should have PREVIEW_LINES lines");
        *last = truncate(&format!("{last} …"), width);
        *len = (*len).min(width.saturating_sub(1));
    }
    lines
}

/// Styles `rows`, which `wrap` made from `text`, drawing the chars that
/// `marked` (indexed by char of `text`) flags black on orange. Each row says
/// how many of its leading chars come from `text`; the rest is never marked.
/// A row with nothing marked comes out exactly as `Line::styled` would make it.
fn highlight_rows(
    text: &str,
    marked: &[bool],
    rows: Vec<(String, usize)>,
    style: Style,
) -> Vec<Line<'static>> {
    let chars: Vec<char> = text.chars().collect();
    let is_marked = |i: usize| marked.get(i).copied().unwrap_or(false);
    // Walks `text` alongside the rows. `wrap` drops whitespace and joins the
    // words of a row with one space, which stands for the run it replaced.
    let mut cursor = 0;
    rows.into_iter()
        .map(|(row, from_text)| {
            let flags: Vec<bool> = row
                .chars()
                .enumerate()
                .map(|(n, ch)| {
                    if n >= from_text {
                        false
                    } else if ch.is_whitespace() {
                        is_marked(cursor)
                    } else {
                        while chars.get(cursor).is_some_and(|c| c.is_whitespace()) {
                            cursor += 1;
                        }
                        cursor += 1;
                        is_marked(cursor - 1)
                    }
                })
                .collect();
            if !flags.contains(&true) {
                return Line::styled(row, style);
            }
            let hit = Style::new().fg(Color::Black).bg(ORANGE);
            let mut spans: Vec<Span> = Vec::new();
            let mut run = String::new();
            for (n, ch) in row.chars().enumerate() {
                run.push(ch);
                if flags.get(n + 1) != Some(&flags[n]) {
                    let run = std::mem::take(&mut run);
                    spans.push(if flags[n] {
                        Span::styled(run, hit)
                    } else {
                        Span::raw(run)
                    });
                }
            }
            Line::from(spans).style(style)
        })
        .collect()
}

fn card_height(post: &Post, width: usize) -> u16 {
    let lines = 1 + wrap(post.title, width).len() + preview(post, width).len() + 1;
    lines as u16 + 2
}

/// The group/author/age/flair row, shortened to fit `width`: first "Posted
/// by" goes, then the author, then the flair, and only then is it truncated.
fn meta_line(app: &App, post: &Post, width: usize) -> Line<'static> {
    let group = format!("w/{}", app.groups[post.group].name);
    let age = age(post.hours_ago);
    let full = format!(" · Posted by u/{} · {age}", post.author);
    let short = format!(" · u/{} · {age}", post.author);
    let minimal = format!(" · {age}");
    let flair = post.flair.map(|f| {
        [
            Span::raw("  "),
            Span::styled(
                format!(" {f} "),
                Style::new().fg(Color::Black).bg(flair_color(f)),
            ),
        ]
    });

    let group_style = Style::new().fg(Color::White).bold();
    let build = |byline: &str, flair: Option<&[Span<'static>; 2]>| {
        let mut spans = vec![
            Span::styled(group.clone(), group_style),
            Span::styled(byline.to_owned(), Style::new().fg(DIM)),
        ];
        spans.extend(flair.into_iter().flatten().cloned());
        Line::from(spans)
    };

    [
        build(&full, flair.as_ref()),
        build(&short, flair.as_ref()),
        build(&minimal, flair.as_ref()),
        build(&minimal, None),
    ]
    .into_iter()
    .find(|line| line.width() <= width)
    .unwrap_or_else(|| Line::styled(truncate(&format!("{group}{minimal}"), width), group_style))
}

fn flair_color(flair: &str) -> Color {
    match flair {
        "Official" | "PSA" => Color::Rgb(255, 214, 53),
        "Win" | "Demo" => Color::Rgb(70, 209, 96),
        "Rant" | "Help" => Color::Rgb(255, 88, 91),
        "Pics" | "Meme" | "Story" => Color::Rgb(255, 153, 204),
        _ => Color::Rgb(160, 180, 255),
    }
}

/// The action row, and the columns of its Save label within it.
fn actions_line(post: &Post) -> (Line<'static>, Range<u16>) {
    let dim = Style::new().fg(DIM).bold();
    let save = if post.saved {
        Span::styled("★ Saved", Style::new().fg(ORANGE).bold())
    } else {
        Span::styled("☆ Save", dim)
    };
    let mut spans = vec![
        Span::styled(
            format!("▤ {} Comments", compact(i64::from(post.comment_count))),
            dim,
        ),
        Span::raw("   "),
        Span::styled("↗ Share", dim),
        Span::raw("   "),
    ];
    let start = Line::from(spans.clone()).width() as u16;
    let save_columns = start..start + save.width() as u16;
    spans.extend([save, Span::raw("   "), Span::styled("···", dim)]);
    (Line::from(spans), save_columns)
}

/// Draws the card for `view[index]`, recording its arrows and Save label.
fn render_card(
    frame: &mut Frame,
    app: &App,
    index: usize,
    post: &Post,
    area: Rect,
    hits: &mut Hits,
) {
    let selected = index == app.selected;
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(if selected { ORANGE } else { BORDER });
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [votes, content] =
        Layout::horizontal([Constraint::Length(VOTE_WIDTH), Constraint::Min(0)]).areas(inner);

    let color = vote_color(post.vote);
    frame.render_widget(
        Paragraph::new(vec![
            Line::raw(""),
            Line::styled(
                compact(i64::from(post.score)),
                Style::new().fg(color).bold(),
            ),
        ])
        .centered()
        .bg(if selected { HIGHLIGHT_BG } else { Color::Reset }),
        votes,
    );
    // The arrows go in the cells a centered one-cell line would, drawn one by
    // one so that the cell drawn is the cell recorded.
    for (row, glyph, vote, c) in [(0, "▲", Vote::Up, ORANGE), (2, "▼", Vote::Down, PERIWINKLE)]
    {
        let cell = Rect::new(votes.x + votes.width / 2, votes.y + row, 1, 1);
        if votes.width == 0 || cell.y >= votes.bottom() {
            continue;
        }
        let active = post.vote == vote;
        frame.render_widget(
            Span::styled(glyph, Style::new().fg(if active { c } else { DIM }).bold()),
            cell,
        );
        hits.regions.push((cell, Target::Vote(index, vote)));
    }

    let width = usize::from(content.width.saturating_sub(2));
    let title_style = Style::new().fg(Color::White).bold();
    let lines: Vec<Line> = std::iter::once(meta_line(app, post, width))
        .chain(highlight_rows(
            post.title,
            &app.search.highlights(post.title),
            whole(wrap(post.title, width)),
            title_style,
        ))
        .chain(highlight_rows(
            post.body,
            &app.search.highlights(post.body),
            preview(post, width),
            Style::new().fg(Color::Gray),
        ))
        .collect();
    let (actions, save) = actions_line(post);
    let text = Rect {
        x: content.x + 1,
        width: content.width.saturating_sub(1),
        ..content
    };
    // The action row is the last line, unless the card is clipped above it.
    let y = text.y + lines.len() as u16;
    let save = Rect::new(text.x + save.start, y, save.end - save.start, 1).intersection(text);
    if !save.is_empty() {
        hits.regions.push((save, Target::Save(index)));
    }
    let lines: Vec<Line> = lines.into_iter().chain([actions]).collect();
    frame.render_widget(Paragraph::new(lines), text);
}

/// One comment, without its replies. A collapsed one is only its header,
/// marked `[+]` and counting the replies it hides, and its guide row.
fn comment_lines(
    comment: &Comment,
    depth: usize,
    collapsed: bool,
    focused: bool,
    width: usize,
    out: &mut Vec<Line<'static>>,
) {
    let guide = Style::new().fg(BORDER);
    let prefix = "│ ".repeat(depth);
    let text_width = width.saturating_sub(prefix.chars().count());

    let mut header = vec![Span::styled(prefix.clone(), guide)];
    if collapsed {
        header.push(Span::styled("[+] ", Style::new().fg(DIM)));
    }
    header.extend([
        Span::styled(format!("u/{}", comment.author), Style::new().bold()),
        Span::styled(
            format!(" · {}", age(comment.hours_ago)),
            Style::new().fg(DIM),
        ),
    ]);
    let replies = comment.total() - 1;
    if collapsed && replies > 0 {
        let noun = if replies == 1 { "reply" } else { "replies" };
        header.push(Span::styled(
            format!(" ({replies} {noun})"),
            Style::new().fg(DIM),
        ));
    }
    if focused {
        for span in &mut header[1..] {
            span.style = Style::new().fg(ORANGE).bg(HIGHLIGHT_BG).bold();
        }
    }
    out.push(Line::from(header));

    if !collapsed {
        out.extend(
            wrap(comment.body, text_width)
                .into_iter()
                .map(|l| Line::from(vec![Span::styled(prefix.clone(), guide), Span::raw(l)])),
        );
        let arrow = |active: bool, glyph: &'static str, c: Color| {
            Span::styled(glyph, Style::new().fg(if active { c } else { DIM }).bold())
        };
        let score = match comment.vote {
            Vote::None => DIM,
            vote => vote_color(vote),
        };
        out.push(Line::from(vec![
            Span::styled(prefix.clone(), guide),
            arrow(comment.vote == Vote::Up, "▲", ORANGE),
            Span::styled(
                format!(" {} ", compact(i64::from(comment.score))),
                Style::new().fg(score).bold(),
            ),
            arrow(comment.vote == Vote::Down, "▼", PERIWINKLE),
            Span::styled("   Reply   Share", Style::new().fg(DIM).bold()),
        ]));
    }
    out.push(Line::styled(format!("{prefix}│"), guide));
}

fn render_post(frame: &mut Frame, app: &mut App, area: Rect, scroll: u16, hits: &mut Hits) {
    let Some(post) = app.selected_post() else {
        return;
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(ORANGE);
    let inner = block.inner(area);
    let text = block.clone().padding(Padding::horizontal(1)).inner(area);
    let width = usize::from(inner.width.saturating_sub(2));

    let color = vote_color(post.vote);
    let mut lines: Vec<Line> = vec![meta_line(app, post, width), Line::raw("")];
    lines.extend(highlight_rows(
        post.title,
        &app.search.highlights(post.title),
        whole(wrap(post.title, width)),
        Style::new().fg(Color::White).bold(),
    ));
    lines.push(Line::raw(""));
    lines.extend(highlight_rows(
        post.body,
        &app.search.highlights(post.body),
        whole(wrap(post.body, width)),
        Style::default(),
    ));
    lines.push(Line::raw(""));
    let (mut action, _) = actions_line(post);
    action.spans.splice(
        0..0,
        [
            Span::styled(
                "▲ ",
                Style::new().fg(if post.vote == Vote::Up { ORANGE } else { DIM }),
            ),
            Span::styled(
                compact(i64::from(post.score)),
                Style::new().fg(color).bold(),
            ),
            Span::styled(
                " ▼",
                Style::new().fg(if post.vote == Vote::Down {
                    PERIWINKLE
                } else {
                    DIM
                }),
            ),
            Span::raw("   "),
        ],
    );
    lines.push(action);
    lines.push(Line::styled("─".repeat(width), Style::new().fg(BORDER)));
    lines.push(Line::from(vec![
        Span::styled("Sort by: ", Style::new().fg(DIM)),
        Span::styled("Best ▾", Style::new().bold()),
    ]));
    lines.push(Line::raw(""));

    let mut focus_row = None;
    let mut headers: Vec<(u16, CommentPath)> = Vec::new();
    for shown in app.thread.visible(&post.comments) {
        let focused = app.thread.focus.as_ref() == Some(&shown.path);
        if focused {
            focus_row = Some(lines.len() as u16);
        }
        headers.push((lines.len() as u16, shown.path.clone()));
        comment_lines(
            shown.comment,
            shown.depth(),
            shown.collapsed,
            focused,
            width,
            &mut lines,
        );
    }
    // Counts the whole seeded tree, so collapsing doesn't change it.
    let shown: u32 = post.comments.iter().map(Comment::total).sum();
    let hidden = post.comment_count.saturating_sub(shown);
    if hidden > 0 {
        lines.push(Line::styled(
            format!("··· {hidden} more comments"),
            Style::new().fg(PERIWINKLE).bold(),
        ));
    }

    let max_scroll = (lines.len() as u16).saturating_sub(inner.height);
    let mut scroll = scroll.min(max_scroll);
    // Right after the focus moves, scroll the least that shows its header.
    if app.thread.reveal
        && let Some(row) = focus_row
    {
        scroll = scroll.clamp(row.saturating_sub(inner.height.saturating_sub(1)), row);
    }
    app.thread.reveal = false;
    app.mode = Mode::Post { scroll };

    hits.max_scroll = max_scroll;
    for (row, path) in headers {
        if let Some(y) = row.checked_sub(scroll).filter(|&dy| dy < text.height) {
            let rect = Rect {
                y: text.y + y,
                height: 1,
                ..text
            };
            hits.regions.push((rect, Target::Comment(path)));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .scroll((scroll, 0))
            .block(block.padding(Padding::horizontal(1))),
        area,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::crossterm::event::KeyCode;
    use ratatui::crossterm::event::KeyEvent;
    use ratatui::crossterm::event::KeyModifiers;
    use ratatui::crossterm::event::MouseButton;
    use ratatui::crossterm::event::MouseEvent;
    use ratatui::crossterm::event::MouseEventKind;

    use super::*;
    use crate::data;

    fn buffer(app: &mut App, width: u16, height: u16) -> Buffer {
        let mut terminal =
            Terminal::new(TestBackend::new(width, height)).expect("test backend is infallible");
        terminal
            .draw(|f| render(f, app))
            .expect("test backend is infallible");
        terminal.backend().buffer().clone()
    }

    fn rows(buf: &Buffer) -> Vec<String> {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    fn screen(app: &mut App, width: u16, height: u16) -> String {
        rows(&buffer(app, width, height)).join("\n")
    }

    #[test]
    fn helpers() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1_000), "1k", "trailing .0 is trimmed");
        assert_eq!(compact(1_243), "1.2k");
        assert_eq!(compact(-1_500), "-1.5k");
        assert_eq!(age(0), "just now");
        assert_eq!(age(23), "23h ago");
        assert_eq!(age(50), "2d ago");
        assert_eq!(wrap("aa bb cc", 5), ["aa bb", "cc"]);
        assert_eq!(wrap("abcdefg", 3), ["abc", "def", "g"]);
        assert_eq!(
            wrap("aa ño", 5),
            ["aa ño"],
            "5 chars fit in 5 columns even though \"ño\" is 3 bytes"
        );
        assert_eq!(truncate("hello", 4), "hel…");
        assert_eq!(truncate("hi", 4), "hi");
    }

    #[test]
    fn feed_renders_like_a_frontpage() {
        let mut app = App::new();
        let first: String = app
            .selected_post()
            .expect("seed data is not empty")
            .title
            .chars()
            .take(20)
            .collect();
        let s = screen(&mut app, 140, 40);
        for needle in [
            "workit",
            "Hot",
            "Rising",
            "w/Engineering",
            "Trending today",
            &first,
        ] {
            assert!(s.contains(needle), "missing {needle:?} in:\n{s}");
        }
    }

    #[test]
    fn narrow_terminal_hides_sidebars() {
        let mut app = App::new();
        let s = screen(&mut app, 60, 30);
        assert!(!s.contains("Trending today"));
        assert!(!s.contains("GROUPS"));
        assert!(s.contains("Hot"));
    }

    #[test]
    fn selected_card_stays_visible() {
        let mut app = App::new();
        app.on_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
        let last: String = app
            .selected_post()
            .expect("seed data is not empty")
            .title
            .chars()
            .take(20)
            .collect();
        let s = screen(&mut app, 140, 30);
        assert!(app.offset > 0, "should have scrolled down");
        assert!(s.contains(&last), "last post missing:\n{s}");
    }

    fn meta_text(app: &App, post: &Post, width: usize) -> String {
        meta_line(app, post, width)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    }

    #[test]
    fn meta_line_shortens_in_order_and_never_overflows() {
        let app = App::new();
        let post = app
            .posts
            .iter()
            .find(|p| p.flair.is_some())
            .expect("seed data has flaired posts");
        let flair = post.flair.expect("just checked");
        let full = meta_line(&app, post, usize::MAX).width();

        let wide = meta_text(&app, post, full);
        assert!(wide.contains("Posted by") && wide.contains(flair), "{wide}");

        let medium = meta_text(&app, post, full - 1);
        assert!(
            !medium.contains("Posted by"),
            "Posted by goes first: {medium}"
        );
        assert!(medium.contains(post.author) && medium.contains(flair));

        let author_dropped = (0..full)
            .rev()
            .map(|w| meta_text(&app, post, w))
            .find(|t| !t.contains(post.author))
            .expect("author is dropped at some width");
        assert!(
            author_dropped.contains(flair),
            "author goes before flair: {author_dropped}"
        );

        for w in 1..=full {
            assert!(
                meta_line(&app, post, w).width() <= w,
                "overflow at width {w}"
            );
        }
    }

    #[test]
    fn short_terminal_skips_sidebar_boxes_instead_of_squeezing() {
        let mut app = App::new();
        let s = screen(&mut app, 140, 24);
        assert!(s.contains("Create Post"), "About box must stay whole:\n{s}");
        assert!(s.contains("Trending today"));
        assert!(!s.contains("Popular groups"), "no room left for it:\n{s}");

        let s = screen(&mut app, 140, 40);
        for needle in ["Create Post", "Trending today", "Popular groups"] {
            assert!(s.contains(needle), "missing {needle:?} at 40 rows:\n{s}");
        }
    }

    #[test]
    fn empty_feed_message_depends_on_search() {
        let mut app = App::new();
        app.view.clear();
        let s = screen(&mut app, 140, 30);
        assert!(
            s.contains("Nothing here yet"),
            "empty feed without a search:\n{s}"
        );
        assert!(!s.contains("spelling"));

        app.query.push_str("zzz");
        let s = screen(&mut app, 140, 30);
        assert!(
            s.contains("couldn't find any results"),
            "empty search:\n{s}"
        );
    }

    #[test]
    fn post_view_shows_comments_and_clamps_scroll() {
        let mut app = App::new();
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let author = app
            .selected_post()
            .expect("seed data is not empty")
            .comments[0]
            .author;
        let s = screen(&mut app, 140, 60);
        assert!(
            s.contains(&format!("u/{author}")),
            "first comment missing:\n{s}"
        );
        assert!(s.contains("more comments"));

        app.on_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
        screen(&mut app, 140, 20);
        let Mode::Post { scroll } = app.mode else {
            panic!("should still be in post view");
        };
        assert!(
            scroll < u16::MAX,
            "scroll should be clamped by the renderer"
        );
    }

    fn press(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn nav_shows_saved_after_all() {
        let mut app = App::new();
        let s = screen(&mut app, 110, 24);
        let row = |needle: &str| {
            s.lines()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("missing {needle:?} in:\n{s}"))
        };
        assert_eq!(row("☆ Saved"), row("◎ All") + 1);

        press(&mut app, KeyCode::Char(']'));
        press(&mut app, KeyCode::Char(']'));
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.feed, Feed::Saved);
        let s = screen(&mut app, 110, 24);
        for needle in ["╭ Saved ", "Your personal workit", "Create Post"] {
            assert!(s.contains(needle), "missing {needle:?} in:\n{s}");
        }
    }

    #[test]
    fn join_button_and_nav_mark_follow_membership() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('['));
        let Feed::Group(g) = app.feed else {
            panic!("[ from Home goes to the last group");
        };
        assert!(app.groups[g].joined, "the last seed group starts joined");
        let name = app.groups[g].name;

        let s = screen(&mut app, 110, 24);
        assert!(s.contains(" Joined "), "{s}");
        assert!(s.contains(&format!("● w/{name}")), "{s}");

        press(&mut app, KeyCode::Char('J'));
        let s = screen(&mut app, 110, 24);
        assert!(s.contains(" Join ") && !s.contains("Joined"), "{s}");
        assert!(s.contains(&format!("○ w/{name}")), "{s}");
        assert!(s.contains("Left group"), "toast:\n{s}");

        press(&mut app, KeyCode::Char('J'));
        let s = screen(&mut app, 110, 24);
        assert!(s.contains(" Joined "), "{s}");
        assert!(s.contains(&format!("● w/{name}")), "{s}");
    }

    #[test]
    fn empty_saved_feed_message() {
        let mut app = App::new();
        app.feed = Feed::Saved;
        press(&mut app, KeyCode::Char('1'));
        assert!(app.view.is_empty(), "nothing is saved at startup");
        let s = screen(&mut app, 110, 24);
        assert!(s.contains("No saved posts yet"), "{s}");
        assert!(s.contains("Press s on any post to save it"), "{s}");
        assert!(!s.contains("Nothing here yet"));

        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('z'));
        let s = screen(&mut app, 110, 24);
        assert!(s.contains("couldn't find any results"), "{s}");
        assert!(!s.contains("No saved posts yet"));
        press(&mut app, KeyCode::Esc);

        // Saved posts that Rising filters out are not "no saved posts".
        let old = app
            .posts
            .iter()
            .position(|p| p.hours_ago >= 24)
            .expect("seed data has posts a day old");
        app.posts[old].saved = true;
        press(&mut app, KeyCode::Char('4'));
        assert!(app.view.is_empty());
        let s = screen(&mut app, 110, 24);
        assert!(s.contains("Nothing here yet"), "{s}");
        assert!(!s.contains("No saved posts yet"));
    }

    #[test]
    fn every_hint_set_fits_80_columns_on_every_feed() {
        let mut app = App::new();
        app.posts[0].saved = true;
        // So that `n` has a comment to focus on every post.
        for post in app.posts.iter_mut().filter(|p| p.comments.is_empty()) {
            post.comments.push(data::c("someone", 1, 1, "First!"));
        }
        let footer = |app: &mut App| {
            let s = screen(app, 80, 24);
            s.lines()
                .last()
                .expect("screen has rows")
                .trim_end()
                .to_owned()
        };

        let feeds = 4 + app.groups.len();
        for _ in 0..feeds {
            let feed = app.feed;
            let on_group = matches!(feed, Feed::Group(_));

            let plain = footer(&mut app);
            assert!(plain.ends_with("q quit"), "{feed:?}: {plain:?}");
            assert_eq!(plain.contains("J join"), on_group, "{feed:?}: {plain:?}");

            press(&mut app, KeyCode::Char('/'));
            press(&mut app, KeyCode::Char('e'));
            let search = footer(&mut app);
            assert!(search.ends_with("esc clear"), "{feed:?}: {search:?}");
            assert_eq!(
                search, " type filter  w/ u/ flair: narrow  enter keep  esc clear",
                "{feed:?}"
            );

            press(&mut app, KeyCode::Enter);
            let kept = footer(&mut app);
            assert!(
                kept.contains("esc clear search") && kept.ends_with("q quit"),
                "{feed:?}: {kept:?}"
            );
            press(&mut app, KeyCode::Esc);

            assert!(!app.view.is_empty(), "{feed:?} needs a post to open");
            press(&mut app, KeyCode::Enter);
            let post = footer(&mut app);
            assert!(post.ends_with("esc back"), "{feed:?}: {post:?}");
            assert_eq!(
                post, " j/k scroll  n/p comments  a/z vote  s save  c share  esc back",
                "{feed:?}"
            );

            press(&mut app, KeyCode::Char('n'));
            assert!(app.thread.focus.is_some(), "{feed:?}");
            let focused = footer(&mut app);
            assert_eq!(
                focused, " j/k scroll  n/p comments  a/z vote comment  enter collapse  esc unfocus",
                "{feed:?}"
            );
            press(&mut app, KeyCode::Char('c'));
            assert_eq!(footer(&mut app), " ✔ Link copied to clipboard!", "{feed:?}");
            press(&mut app, KeyCode::Esc);
            assert_eq!(footer(&mut app), post, "{feed:?}: unfocused again");
            press(&mut app, KeyCode::Esc);
            assert_eq!(app.mode, Mode::Feed, "{feed:?}");

            press(&mut app, KeyCode::Char(']'));
        }
        assert_eq!(app.feed, Feed::Home, "visited every feed");
    }

    fn type_str(app: &mut App, text: &str) {
        text.chars().for_each(|ch| press(app, KeyCode::Char(ch)));
    }

    /// The seed app with its posts swapped for one Engineering post, on Home.
    fn one_post(title: &'static str, body: &'static str) -> App {
        let mut app = App::new();
        app.posts = vec![Post {
            group: 0,
            author: "dana_k",
            title,
            body,
            score: 10,
            hours_ago: 1,
            ..Default::default()
        }];
        press(&mut app, KeyCode::Char('1'));
        assert_eq!(app.view, [0]);
        app
    }

    /// Every (x, y) where `needle` starts, in reading order. All glyphs here
    /// are one cell wide, so a char index is a column.
    fn find_all(buf: &Buffer, needle: &str) -> Vec<(u16, u16)> {
        rows(buf)
            .iter()
            .enumerate()
            .flat_map(|(y, row)| {
                row.match_indices(needle)
                    .map(|(at, _)| (row[..at].chars().count() as u16, y as u16))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn find(buf: &Buffer, needle: &str) -> (u16, u16) {
        *find_all(buf, needle)
            .first()
            .unwrap_or_else(|| panic!("missing {needle:?} in:\n{}", rows(buf).join("\n")))
    }

    fn is_hit(buf: &Buffer, x: u16, y: u16) -> bool {
        let cell = &buf[(x, y)];
        cell.fg == Color::Black && cell.bg == ORANGE
    }

    /// One char per cell from `x` for `n` cells: `^` highlighted, `.` not.
    fn hits(buf: &Buffer, x: u16, y: u16, n: u16) -> String {
        (x..x + n)
            .map(|x| if is_hit(buf, x, y) { '^' } else { '.' })
            .collect()
    }

    #[test]
    fn title_match_is_black_on_orange_bold_in_original_case() {
        let mut app = one_post("Old huge post", "Nothing to see.");
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "HUGE");
        let buf = buffer(&mut app, 60, 20);
        let (x, y) = find(&buf, "Old huge post");
        assert_eq!(hits(&buf, x, y, 13), "....^^^^.....", "case is preserved");
        for dx in 0..13 {
            let cell = &buf[(x + dx, y)];
            assert!(cell.modifier.contains(Modifier::BOLD), "title stays bold");
            if !is_hit(&buf, x + dx, y) {
                assert_eq!((cell.fg, cell.bg), (Color::White, Color::Reset));
            }
        }
        assert!(
            find_all(&buf, "HUGE").len() == 1,
            "the header shows the raw query"
        );

        press(&mut app, KeyCode::Enter);
        let kept = buffer(&mut app, 60, 20);
        assert_eq!(hits(&kept, x, y, 13), "....^^^^.....", "kept search");
    }

    #[test]
    fn phrase_wrapping_across_title_lines_is_highlighted_on_both() {
        // 30 columns leave 20 for the card text: "aaaa bbbb cccc dddd" / "eeee".
        let mut app = one_post("aaaa bbbb cccc dddd eeee", "");
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, r#""dddd   eeee""#);
        assert_eq!(app.view, [0]);
        let buf = buffer(&mut app, 30, 20);
        let (x, y) = find(&buf, "aaaa bbbb cccc dddd");
        assert_eq!(
            hits(&buf, x, y, 20),
            "...............^^^^.",
            "no hit past the break"
        );
        assert_eq!(find(&buf, "eeee"), (x, y + 1));
        assert_eq!(hits(&buf, x, y + 1, 5), "^^^^.");
    }

    #[test]
    fn overlapping_terms_make_one_run() {
        let mut app = one_post("dead code here", "");
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, r#"dead "ad co" code"#);
        let buf = buffer(&mut app, 60, 20);
        let (x, y) = find(&buf, "dead code here");
        assert_eq!(hits(&buf, x, y, 15), "^^^^^^^^^......");

        let lines = highlight_rows(
            "dead code here",
            &app.search.highlights("dead code here"),
            whole(vec!["dead code here".to_owned()]),
            Style::new(),
        );
        let spans: Vec<&str> = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(spans, ["dead code", " here"], "drawn once, as one run");
    }

    #[test]
    fn non_ascii_matches_highlight_exact_cells() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "°f");
        let buf = buffer(&mut app, 110, 24);
        let (x, y) = find(&buf, "64°F");
        assert_eq!(hits(&buf, x, y, 5), "..^^.");

        // In the post view the body match lights up; the comment's doesn't.
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "JALAPEÑO");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        let buf = buffer(&mut app, 110, 60);
        let found = find_all(&buf, "jalapeño");
        assert!(found.len() >= 2, "body and a comment: {found:?}");
        let (x, y) = found[0];
        assert_eq!(hits(&buf, x - 1, y, 10), ".^^^^^^^^.");
        let (x, y) = found[found.len() - 1];
        assert_eq!(
            hits(&buf, x, y, 8),
            "........",
            "comments aren't highlighted"
        );
    }

    #[test]
    fn lowercase_expanding_chars_dont_panic() {
        let mut app = one_post("İstanbul İİ office", "İ\nİİ İstanbul İ");
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "İ i̇s");
        let buf = buffer(&mut app, 60, 20);
        let (x, y) = find(&buf, "İstanbul İİ office");
        assert_eq!(hits(&buf, x, y, 18), "^^.......^^.......");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        buffer(&mut app, 60, 20);
        buffer(&mut app, 12, 20);
    }

    #[test]
    fn match_cut_by_preview_ellipsis_highlights_only_visible_part() {
        // 20 columns: "one two three four" / "five six seven eigh…", "nine ten" cut.
        let mut app = one_post("Title", "one two three four five six seven eight nine ten");
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "eight ten");
        assert_eq!(app.view, [0], "ten is past the cut but still matches");
        let buf = buffer(&mut app, 30, 20);
        let (x, y) = find(&buf, "five six seven eigh…");
        assert_eq!(hits(&buf, x, y, 21), "...............^^^^..");
        assert_eq!(
            buf[(x + 19, y)].fg,
            Color::Gray,
            "the … keeps the preview style"
        );

        // " …" appended to a shorter row: neither the space nor `…` is marked.
        let mut app = one_post("Title", "one two three four five six seven xxxxxxxxxx");
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, r#""seven xxx""#);
        let buf = buffer(&mut app, 30, 20);
        let (x, y) = find(&buf, "five six seven …");
        assert_eq!(hits(&buf, x, y, 17), ".........^^^^^...");
    }

    #[test]
    fn filters_alone_render_exactly_like_no_query() {
        let mut app = App::new();
        type_str(&mut app, "]]]]");
        assert_eq!(app.feed, Feed::Group(0));
        let view = app.view.clone();
        let plain_feed = buffer(&mut app, 110, 24);
        press(&mut app, KeyCode::Enter);
        let plain_post = buffer(&mut app, 110, 24);
        press(&mut app, KeyCode::Esc);

        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "W/eng u/ flair:");
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.view, view, "every Engineering post still matches");
        let filtered_feed = buffer(&mut app, 110, 24);
        press(&mut app, KeyCode::Enter);
        let filtered_post = buffer(&mut app, 110, 24);

        // Everything between the header (search box) and the footer (hints).
        for (plain, filtered) in [(plain_feed, filtered_feed), (plain_post, filtered_post)] {
            for y in 3..23 {
                for x in 0..110 {
                    assert_eq!(plain[(x, y)], filtered[(x, y)], "cell ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn highlighting_never_changes_layout() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "the");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('G'));
        let lit = buffer(&mut app, 110, 24);
        let offset = app.offset;
        assert!(offset > 0, "should have scrolled");
        let search = std::mem::take(&mut app.search);
        let unlit = buffer(&mut app, 110, 24);
        assert_eq!(rows(&lit), rows(&unlit));
        assert_eq!(app.offset, offset);
        assert_ne!(lit, unlit, "only the styles differ");

        app.search = search;
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('G'));
        let lit = buffer(&mut app, 110, 24);
        let mode = app.mode;
        app.search = Default::default();
        let unlit = buffer(&mut app, 110, 24);
        assert_eq!(rows(&lit), rows(&unlit));
        assert_eq!(app.mode, mode, "same scroll clamp");
    }

    #[test]
    fn whitespace_only_phrase_draws_no_highlights() {
        let mut plain = one_post("dead code here", "some body text");
        let expected = buffer(&mut plain, 60, 20);
        let mut app = one_post("dead code here", "some body text");
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, r#"" " "   "#);
        assert_eq!(app.view, [0], "the post still matches");
        assert!(app.search.is_empty());
        let buf = buffer(&mut app, 60, 20);
        let (x, y) = find(&buf, "dead code here");
        assert_eq!(hits(&buf, x, y, 14), "..............");
        for y in 3..19 {
            for x in 0..60 {
                assert_eq!(buf[(x, y)], expected[(x, y)], "cell ({x}, {y})");
            }
        }
    }

    #[test]
    fn empty_feed_message_ignores_query_without_terms() {
        let mut app = App::new();
        type_str(&mut app, "]]]");
        assert_eq!(app.feed, Feed::Saved);
        for query in ["w/", "   ", r#"" ""#, "u/ flair:"] {
            press(&mut app, KeyCode::Char('/'));
            type_str(&mut app, query);
            assert!(app.view.is_empty(), "nothing is saved");
            let s = screen(&mut app, 110, 24);
            assert!(s.contains("No saved posts yet"), "{query:?}:\n{s}");
            assert!(!s.contains("couldn't find any results"), "{query:?}");
            press(&mut app, KeyCode::Enter);
            let s = screen(&mut app, 110, 24);
            assert!(s.contains("No saved posts yet"), "kept {query:?}:\n{s}");
            press(&mut app, KeyCode::Esc);
        }

        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "w/eng");
        let s = screen(&mut app, 110, 24);
        assert!(
            s.contains("couldn't find any results"),
            "a real filter:\n{s}"
        );
        assert!(!s.contains("No saved posts yet"));
        press(&mut app, KeyCode::Esc);

        // Rising hides the only saved post; `w/` doesn't change the reason.
        let old = app
            .posts
            .iter()
            .position(|p| p.hours_ago >= 24)
            .expect("seed data has posts a day old");
        app.posts[old].saved = true;
        press(&mut app, KeyCode::Char('4'));
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "w/");
        assert!(app.view.is_empty());
        let s = screen(&mut app, 110, 24);
        assert!(s.contains("Nothing here yet"), "{s}");
        assert!(!s.contains("couldn't find any results"));
    }

    /// `one_post` with the comments ann (bob (cat)), dan (eve) and fay, all
    /// scored 5 and 1h old, and 4 more comments than the 6 seeded.
    fn thread_post() -> App {
        let mut app = one_post("Thread title", "Thread body.");
        let c = |author, body| data::c(author, 5, 1, body);
        app.posts[0].comments = vec![
            c("ann", "ann says hi").with(vec![
                c("bob", "bob replies").with(vec![c("cat", "cat replies")]),
            ]),
            c("dan", "dan says hi").with(vec![c("eve", "eve replies")]),
            c("fay", "fay says hi"),
        ];
        app.posts[0].comment_count = 10;
        app
    }

    fn scroll(app: &App) -> u16 {
        let Mode::Post { scroll } = app.mode else {
            panic!("should be in the post view");
        };
        scroll
    }

    fn is_focus_style(buf: &Buffer, x: u16, y: u16) -> bool {
        let cell = &buf[(x, y)];
        cell.fg == ORANGE && cell.bg == HIGHLIGHT_BG && cell.modifier.contains(Modifier::BOLD)
    }

    #[test]
    fn collapsed_comments_show_header_and_reply_count_only() {
        let mut app = thread_post();
        press(&mut app, KeyCode::Enter);
        let s = screen(&mut app, 110, 40);
        for text in ["ann says hi", "cat replies", "eve replies", "fay says hi"] {
            assert!(s.contains(text), "missing {text:?}:\n{s}");
        }
        assert!(!s.contains("[+]") && !s.contains("[–]") && !s.contains("[-]"));

        // Collapse ann, then dan (n skips bob and cat), then fay.
        type_str(&mut app, "n");
        press(&mut app, KeyCode::Enter);
        type_str(&mut app, "n");
        press(&mut app, KeyCode::Enter);
        type_str(&mut app, "n");
        press(&mut app, KeyCode::Enter);
        let buf = buffer(&mut app, 110, 40);
        let s = rows(&buf).join("\n");
        let (x, y) = find(&buf, "[+] u/ann · 1h ago (2 replies)");
        assert_eq!(find(&buf, "[+] u/dan · 1h ago (1 reply)"), (x, y + 2));
        let fay = "[+] u/fay · 1h ago";
        assert_eq!(find(&buf, fay), (x, y + 4));
        let after: String = rows(&buf)[usize::from(y + 4)]
            .chars()
            .skip(usize::from(x) + fay.chars().count())
            .take(3)
            .collect();
        assert_eq!(after, "   ", "no suffix with no replies");
        for row in [y + 1, y + 3, y + 5] {
            assert_eq!(buf[(x, row)].symbol(), "│", "trailing guide row");
        }
        for text in ["says hi", "bob", "cat", "eve", "Reply"] {
            assert!(!s.contains(text), "{text:?} should be hidden:\n{s}");
        }

        // Focus is on fay: its whole header is highlighted, ann's isn't.
        for dx in 0..fay.chars().count() as u16 {
            assert!(is_focus_style(&buf, x + dx, y + 4), "fay header col {dx}");
        }
        assert!(!is_focus_style(&buf, x + 4, y));
        assert_eq!(buf[(x + 4, y)].bg, Color::Reset);
    }

    #[test]
    fn focused_header_and_comment_votes_are_colored() {
        let mut app = thread_post();
        app.posts[0].comments[2].score = 1_243;
        press(&mut app, KeyCode::Enter);
        type_str(&mut app, "na"); // ann up
        type_str(&mut app, "nz"); // bob down
        type_str(&mut app, "nnnna"); // fay up
        assert_eq!(app.posts[0].score, 10, "the post's score is untouched");
        assert_eq!(app.posts[0].vote, Vote::None);
        let buf = buffer(&mut app, 110, 60);

        let votes = find_all(&buf, " ▼   Reply   Share");
        assert_eq!(votes.len(), 6, "one action row per comment");
        // (score text, ▲ color, score color, ▼ color), in display order.
        let expected = [
            ("6", ORANGE, ORANGE, DIM),
            ("4", DIM, PERIWINKLE, PERIWINKLE),
            ("5", DIM, DIM, DIM),
            ("5", DIM, DIM, DIM),
            ("5", DIM, DIM, DIM),
            ("1.2k", ORANGE, ORANGE, DIM),
        ];
        for (&(score_end, y), (score, up, color, down)) in votes.iter().zip(expected) {
            let len = score.chars().count() as u16;
            let x = score_end - len - 2;
            let row: Vec<char> = rows(&buf)[usize::from(y)].chars().collect();
            let text: String = row[usize::from(x)..usize::from(x + 4 + len)]
                .iter()
                .collect();
            assert_eq!(text, format!("▲ {score} ▼"));
            assert_eq!(buf[(x, y)].fg, up, "▲ of {score}");
            assert_eq!(buf[(x + 2, y)].fg, color, "score {score}");
            assert_eq!(buf[(x + 3 + len, y)].fg, down, "▼ of {score}");
        }

        let (x, y) = find(&buf, "u/fay · 1h ago");
        assert!(is_focus_style(&buf, x, y) && is_focus_style(&buf, x + 13, y));
        let (x, y) = find(&buf, "u/bob · 1h ago");
        assert!(!is_focus_style(&buf, x, y));
    }

    #[test]
    fn more_comments_count_ignores_collapse() {
        let mut app = thread_post();
        press(&mut app, KeyCode::Enter);
        let s = screen(&mut app, 110, 50);
        assert!(s.contains("··· 4 more comments"), "{s}");
        type_str(&mut app, "n");
        press(&mut app, KeyCode::Enter);
        let s = screen(&mut app, 110, 50);
        assert!(s.contains("[+] u/ann"), "{s}");
        assert!(s.contains("··· 4 more comments"), "{s}");
    }

    #[test]
    fn collapsing_reclamps_scroll() {
        let mut app = thread_post();
        press(&mut app, KeyCode::Enter);
        // Rendering after every key, like the event loop.
        type_str(&mut app, "n");
        buffer(&mut app, 110, 16);
        type_str(&mut app, "G");
        buffer(&mut app, 110, 16);
        let bottom = scroll(&app);
        press(&mut app, KeyCode::Enter);
        buffer(&mut app, 110, 16);
        assert_eq!(scroll(&app), bottom - 10, "ann lost 10 rows");
        press(&mut app, KeyCode::Char('k'));
        buffer(&mut app, 110, 16);
        assert_eq!(scroll(&app), bottom - 11, "k moves right away");
    }

    #[test]
    fn focus_scrolls_into_view_once() {
        // 16 rows leave 10 for the post, rows 4 to 13; ann starts on row 10
        // of the content, just below.
        let mut app = thread_post();
        press(&mut app, KeyCode::Enter);
        let buf = buffer(&mut app, 110, 16);
        assert!(find_all(&buf, "u/ann").is_empty());

        type_str(&mut app, "n");
        let buf = buffer(&mut app, 110, 16);
        assert_eq!(find(&buf, "u/ann").1, 13, "scrolled just enough");
        assert_eq!(scroll(&app), 1);

        for _ in 0..3 {
            type_str(&mut app, "n");
            buffer(&mut app, 110, 16);
        }
        let buf = buffer(&mut app, 110, 16);
        assert_eq!(find(&buf, "u/dan").1, 13);
        let revealed = scroll(&app);

        type_str(&mut app, "k");
        buffer(&mut app, 110, 16);
        assert_eq!(scroll(&app), revealed - 1, "k moves exactly one line");
        buffer(&mut app, 110, 16);
        assert_eq!(scroll(&app), revealed - 1, "and isn't pulled back");

        // Up: from the bottom, p brings cat's header to the top row.
        type_str(&mut app, "G");
        buffer(&mut app, 110, 16);
        type_str(&mut app, "p");
        let buf = buffer(&mut app, 110, 16);
        assert_eq!(find(&buf, "u/cat").1, 4);
        let at_cat = scroll(&app);

        // dan is already on screen, so focusing it doesn't scroll.
        type_str(&mut app, "n");
        let buf = buffer(&mut app, 110, 16);
        assert_eq!(scroll(&app), at_cat);
        let (x, y) = find(&buf, "u/dan");
        assert!(is_focus_style(&buf, x, y), "dan is focused");
    }

    #[test]
    fn kept_search_highlights_title_but_not_focused_comments() {
        let mut app = one_post("Dead code party", "Body.");
        app.posts[0].comments = vec![
            data::c("dead_eye", 5, 1, "dead code again").with(vec![data::c(
                "x",
                1,
                1,
                "dead reply",
            )]),
        ];
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "dead");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);

        let check = |app: &mut App, header: &str| {
            let buf = buffer(app, 110, 30);
            let (x, y) = find(&buf, "Dead code party");
            assert_eq!(hits(&buf, x, y, 15), "^^^^...........", "{header}");
            let (x, y) = find(&buf, header);
            let n = header.chars().count() as u16;
            assert_eq!(hits(&buf, x, y, n), ".".repeat(usize::from(n)));
            buf
        };
        let buf = check(&mut app, "u/dead_eye");
        let (x, y) = find(&buf, "dead code again");
        assert_eq!(hits(&buf, x, y, 15), "...............");

        type_str(&mut app, "n");
        let buf = check(&mut app, "u/dead_eye");
        let (x, y) = find(&buf, "dead code again");
        assert_eq!(hits(&buf, x, y, 15), "...............", "comment body");
        assert!(is_focus_style(&buf, find(&buf, "u/dead_eye").0, y - 1));

        press(&mut app, KeyCode::Enter);
        check(&mut app, "[+] u/dead_eye · 1h ago (1 reply)");
    }

    #[test]
    fn seed_post_collapses_with_keys_only() {
        let mut app = App::new();
        assert_eq!(
            app.selected_post().expect("seed data is not empty").title,
            "We deleted 40k lines of dead code and the build got 30% faster"
        );
        press(&mut app, KeyCode::Enter);
        let s = screen(&mut app, 110, 60);
        assert!(s.contains("··· 188 more comments"), "{s}");
        assert!(!s.contains("[+]"));

        // n skips the replies of each comment just collapsed.
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('n'));
            press(&mut app, KeyCode::Enter);
        }
        let buf = buffer(&mut app, 110, 60);
        let s = rows(&buf).join("\n");
        let (x, y) = find(&buf, "[+] u/marco_p · 3h ago (1 reply)");
        assert_eq!(
            find(&buf, "[+] u/lin_tran · 2h ago (2 replies)"),
            (x, y + 2)
        );
        let sam = "[+] u/sam_builds · 1h ago";
        assert_eq!(find(&buf, sam), (x, y + 4));
        let after: String = rows(&buf)[usize::from(y + 4)]
            .chars()
            .skip(usize::from(x) + sam.chars().count())
            .take(3)
            .collect();
        assert_eq!(after, "   ", "no suffix without replies");
        assert!(!s.contains("Reply"), "every body is hidden:\n{s}");
        assert!(s.contains("··· 188 more comments"), "{s}");
    }

    /// `one_post` with a thread five levels deep, in display order ann, bob,
    /// cat, dan, eve, fay, gus:
    ///
    /// ```text
    /// ann
    /// │ bob
    /// │ │ cat
    /// │ │ │ dan
    /// │ │ │ │ eve
    /// │ fay
    /// gus
    /// ```
    fn deep_thread() -> App {
        let mut app = one_post(
            "Deep thread",
            "A body long enough to wrap in a narrow post view.",
        );
        let c = |author, body| data::c(author, 5, 1, body);
        app.posts[0].comments = vec![
            c("ann", "Top level, with a body that wraps at small widths.").with(vec![
                c("bob", "Second level.").with(vec![c("cat", "Third level.").with(vec![
                    c("dan", "Fourth level, also long enough to wrap a few times.")
                        .with(vec![c("eve", "Fifth.")]),
                ])]),
                c("fay", "Second level again."),
            ]),
            c("gus", "Last top-level comment."),
        ];
        app.posts[0].comment_count = 100;
        app
    }

    #[test]
    fn tiny_post_view_keeps_focus_on_screen_without_panicking() {
        const ORDER: [&str; 7] = ["ann", "bob", "cat", "dan", "eve", "fay", "gus"];
        for (width, height) in [(40, 12), (30, 8)] {
            let mut app = deep_thread();
            // Presses `code`, then renders, as the event loop does, and checks
            // that `scroll` is within the clamp. After `n` and `p` it also
            // checks that the focused header is on screen; other keys may
            // scroll it away.
            let key = |app: &mut App, code: KeyCode| {
                press(app, code);
                let buf = buffer(app, width, height);
                let size = format!("{width}x{height}");
                assert!(!rows(&buf).join("").contains("GROUPS"), "{size}: no nav");

                let at = scroll(app);
                app.mode = Mode::Post { scroll: u16::MAX };
                buffer(app, width, height);
                let max = scroll(app);
                app.mode = Mode::Post { scroll: at };
                assert!(at <= max, "{size} {code:?}: scroll {at} > {max}");

                let focus = app.thread.focus.clone()?;
                let shown = app.thread.visible(&app.posts[0].comments);
                let author = shown
                    .iter()
                    .find(|s| s.path == focus)
                    .expect("the focus is visible")
                    .comment
                    .author;
                // The post's viewport: below its top border, above its bottom
                // border and the footer.
                let on_screen = find_all(&buf, &format!("u/{author}"))
                    .into_iter()
                    .any(|(x, y)| (4..height - 2).contains(&y) && is_focus_style(&buf, x, y));
                let moved = matches!(code, KeyCode::Char('n' | 'p'));
                assert!(
                    on_screen || !moved,
                    "{size} {code:?}: u/{author} off screen:\n{}",
                    rows(&buf).join("\n")
                );
                Some(author)
            };

            press(&mut app, KeyCode::Enter);
            buffer(&mut app, width, height);
            let walk: Vec<_> = (0..8).map(|_| key(&mut app, KeyCode::Char('n'))).collect();
            assert_eq!(walk[..7], ORDER.map(Some), "{width}x{height}");
            assert_eq!(walk[7], Some("gus"), "n stops at the last comment");
            let back: Vec<_> = (0..7).map(|_| key(&mut app, KeyCode::Char('p'))).collect();
            let mut expected = ORDER.map(Some)[..6].to_vec();
            expected.reverse();
            expected.push(Some("ann"));
            assert_eq!(back, expected, "p walks back and stops at the first");

            // Collapse bob: n skips cat, dan and eve. Then expand it again.
            assert_eq!(key(&mut app, KeyCode::Char('n')), Some("bob"));
            key(&mut app, KeyCode::Enter);
            assert_eq!(key(&mut app, KeyCode::Char('n')), Some("fay"));
            assert_eq!(key(&mut app, KeyCode::Char('p')), Some("bob"));
            key(&mut app, KeyCode::Enter);
            assert_eq!(key(&mut app, KeyCode::Char('n')), Some("cat"));

            // Vote on eve, five levels deep, then scroll around.
            key(&mut app, KeyCode::Char('n'));
            assert_eq!(key(&mut app, KeyCode::Char('n')), Some("eve"));
            key(&mut app, KeyCode::Char('a'));
            let eve = &app.posts[0].comments[0].replies[0].replies[0].replies[0].replies[0];
            assert_eq!((eve.author, eve.score, eve.vote), ("eve", 6, Vote::Up));
            assert_eq!(app.posts[0].score, 10, "the post's score is untouched");
            key(&mut app, KeyCode::Char('g'));
            assert_eq!(key(&mut app, KeyCode::Char('p')), Some("dan"));
            key(&mut app, KeyCode::Char('G'));
            assert_eq!(key(&mut app, KeyCode::Char('p')), Some("cat"));
            key(&mut app, KeyCode::Enter);
            key(&mut app, KeyCode::Char('k'));
            key(&mut app, KeyCode::Esc);
            assert_eq!(
                app.mode,
                Mode::Post {
                    scroll: scroll(&app)
                }
            );
            assert_eq!(app.thread.focus, None);
        }
    }

    fn mouse(app: &mut App, kind: MouseEventKind, (column, row): (u16, u16)) {
        app.on_mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    }

    fn click(app: &mut App, at: (u16, u16)) {
        mouse(app, MouseEventKind::Down(MouseButton::Left), at);
    }

    fn title_of(app: &App, view_index: usize) -> String {
        app.posts[app.view[view_index]]
            .title
            .chars()
            .take(20)
            .collect()
    }

    #[test]
    fn clicking_a_card_after_scrolling_opens_that_post() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('G'));
        buffer(&mut app, 110, 24);
        let offset = app.offset;
        assert!(offset > 0 && offset < app.selected, "scrolled: {offset}");
        press(&mut app, KeyCode::Char('k'));
        let buf = buffer(&mut app, 110, 24);
        assert_eq!(app.offset, offset);

        // The first card drawn is view[offset], not view[0].
        let title = title_of(&app, offset);
        let (x, y) = find(&buf, &title);
        click(&mut app, (x + 3, y));
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
        assert_eq!(app.selected, offset);
        assert_eq!(app.thread, Default::default());

        // A border cell counts too: the card's left edge, beside its title.
        press(&mut app, KeyCode::Esc);
        let buf = buffer(&mut app, 110, 24);
        let title = title_of(&app, offset + 1);
        let (x, y) = find(&buf, &title);
        let border = (x - 8, y);
        assert_eq!(buf[border].symbol(), "│");
        click(&mut app, border);
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
        assert_eq!(app.selected, offset + 1);
    }

    #[test]
    fn clicking_the_visible_part_of_a_clipped_card_opens_it() {
        let mut app = App::new();
        // At 27 rows the third card shows its top border, meta row and two
        // title rows, then is cut off by the bottom of the screen.
        let buf = buffer(&mut app, 110, 27);
        let s = rows(&buf).join("\n");
        let title = title_of(&app, 2);
        let (x, y) = find(&buf, &title);
        assert_eq!(y, 23, "{s}");
        assert_eq!(find_all(&buf, "Comments").len(), 2, "no action row:\n{s}");
        assert_eq!(buf[(x, 25)].symbol(), "─", "the clipped bottom edge");

        click(&mut app, (x, 25));
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
        assert_eq!(app.selected, 2);

        press(&mut app, KeyCode::Esc);
        app.selected = 0;
        buffer(&mut app, 110, 27);
        click(&mut app, (x + 5, y));
        assert_eq!((app.mode, app.selected), (Mode::Post { scroll: 0 }, 2));

        // At 24 rows only its two border rows are left; they still count.
        press(&mut app, KeyCode::Esc);
        app.selected = 0;
        let buf = buffer(&mut app, 110, 24);
        assert_eq!(buf[(x, 21)].symbol(), "─");
        assert_eq!(buf[(x, 22)].symbol(), "─");
        click(&mut app, (x, 22));
        assert_eq!((app.mode, app.selected), (Mode::Post { scroll: 0 }, 2));
    }

    #[test]
    fn clicking_arrows_votes_and_flips_without_opening_or_resorting() {
        let mut app = App::new();
        let view = app.view.clone();
        let i = app.view[1];
        let base = app.posts[i].score;
        let arrow = |app: &mut App, glyph: &str| {
            let buf = buffer(app, 110, 24);
            let found = find_all(&buf, glyph);
            assert_eq!(found.len(), 2, "two cards with arrows");
            found[1]
        };

        let up = arrow(&mut app, "▲");
        click(&mut app, up);
        assert_eq!(app.selected, 1, "the clicked card is selected");
        assert_eq!(
            (app.posts[i].vote, app.posts[i].score),
            (Vote::Up, base + 1)
        );
        assert_eq!(app.mode, Mode::Feed, "the post isn't opened");

        let down = arrow(&mut app, "▼");
        assert_eq!(down, (up.0, up.1 + 2));
        click(&mut app, down);
        assert_eq!(
            (app.posts[i].vote, app.posts[i].score),
            (Vote::Down, base - 1),
            "flipping is -2"
        );
        let buf = buffer(&mut app, 110, 24);
        assert_eq!(buf[down].fg, PERIWINKLE);
        click(&mut app, down);
        assert_eq!(
            (app.posts[i].vote, app.posts[i].score),
            (Vote::None, base),
            "repeating clears it"
        );
        assert_eq!(app.view, view, "voting never re-sorts");
        assert_eq!(app.mode, Mode::Feed);

        // The blank cells beside an arrow belong to the card.
        buffer(&mut app, 110, 24);
        click(&mut app, (up.0 - 1, up.1));
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
        assert_eq!(app.posts[i].vote, Vote::None);
    }

    #[test]
    fn clicking_save_toggles_it_with_a_toast() {
        let mut app = App::new();
        let i = app.view[1];
        let buf = buffer(&mut app, 110, 24);
        // The last match: the nav's "☆ Saved" and card 0 come first.
        let save = *find_all(&buf, "☆ Save").last().expect("card 1 is drawn");
        assert_eq!(find_all(&buf, "☆ Save").len(), 3);
        click(&mut app, (save.0 + 5, save.1));
        assert!(app.posts[i].saved);
        assert_eq!(app.selected, 1);
        assert_eq!(app.mode, Mode::Feed, "the post isn't opened");
        assert_eq!(app.status, Some("Post saved"));
        let buf = buffer(&mut app, 110, 24);
        assert!(rows(&buf)[23].starts_with(" ✔ Post saved "));

        assert_eq!(find(&buf, "★ Saved"), save);
        click(&mut app, (save.0 + 6, save.1));
        assert!(!app.posts[i].saved);
        assert_eq!(app.status, Some("Post unsaved"));
        assert_eq!(app.mode, Mode::Feed);
    }

    #[test]
    fn clicking_a_sort_tab_is_like_its_digit() {
        let mut app = App::new();
        let buf = buffer(&mut app, 110, 24);
        let (x, y) = find(&buf, "3 Top");
        click(&mut app, (x + 4, y));
        assert_eq!(app.sort, Sort::Top);
        let mut by_key = App::new();
        press(&mut by_key, KeyCode::Char('3'));
        assert_eq!(app.view, by_key.view);

        // On the digit, and on the current tab, which resets the selection.
        press(&mut app, KeyCode::Char('j'));
        buffer(&mut app, 110, 24);
        click(&mut app, (x, y));
        assert_eq!((app.sort, app.selected), (Sort::Top, 0));
        let (x, y) = find(&buf, "4 Rising");
        click(&mut app, (x, y));
        assert_eq!(app.sort, Sort::Rising);

        // The gaps between tabs do nothing.
        buffer(&mut app, 110, 24);
        click(&mut app, (x - 2, y));
        assert_eq!(app.sort, Sort::Rising);
    }

    #[test]
    fn clicking_the_nav_switches_feeds() {
        let mut app = App::new();
        let buf = buffer(&mut app, 110, 24);
        // Anywhere on the entry's row inside the nav.
        let (x, y) = find(&buf, "↗ Popular");
        click(&mut app, (x + 15, y));
        assert_eq!(app.feed, Feed::Popular);

        // A kept search stays.
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "the");
        press(&mut app, KeyCode::Enter);
        buffer(&mut app, 110, 24);
        click(&mut app, find(&buf, "◎ All"));
        assert_eq!(app.feed, Feed::All);
        assert_eq!(app.query, "the");
        let mut by_key = App::new();
        type_str(&mut by_key, "]]/the");
        assert_eq!(app.view, by_key.view);

        // The current feed keeps its view and selection.
        press(&mut app, KeyCode::Char('j'));
        let view = app.view.clone();
        buffer(&mut app, 110, 24);
        click(&mut app, find(&buf, "◎ All"));
        assert_eq!((app.feed, app.selected), (Feed::All, 1));
        assert_eq!(app.view, view);

        // The heading, the blank row and the border do nothing.
        let (gx, gy) = find(&buf, "GROUPS");
        for at in [(gx, gy), (gx + 10, gy), (gx, gy - 1), (0, gy), (gx, 3)] {
            buffer(&mut app, 110, 24);
            click(&mut app, at);
            assert_eq!((app.feed, app.selected), (Feed::All, 1), "{at:?}");
            assert_eq!(app.mode, Mode::Feed);
        }

        buffer(&mut app, 110, 24);
        click(&mut app, find(&buf, "w/Foodies"));
        assert_eq!(app.feed, Feed::Group(5));
        assert_eq!(app.groups[5].name, "Foodies");
        buffer(&mut app, 110, 24);
        click(&mut app, find(&buf, "⌂ Home"));
        assert_eq!(app.feed, Feed::Home);
    }

    #[test]
    fn nav_is_not_clickable_when_not_drawn() {
        let mut app = App::new();
        let wide = buffer(&mut app, 110, 30);
        let popular = find(&wide, "↗ Popular");
        let narrow = buffer(&mut app, 60, 30);
        assert!(find_all(&narrow, "Popular").is_empty());
        let hits = app.hits.as_ref().expect("rendered");
        assert!(
            !hits
                .regions
                .iter()
                .any(|(_, t)| matches!(t, Target::Feed(_))),
            "{hits:?}"
        );
        // Where Popular would be is the tab box's bottom border.
        click(&mut app, popular);
        assert_eq!((app.feed, app.mode), (Feed::Home, Mode::Feed));
    }

    #[test]
    fn clicking_the_search_box_enters_search_mode() {
        let mut app = App::new();
        let buf = buffer(&mut app, 110, 24);
        click(&mut app, find(&buf, "Search Home"));
        assert_eq!(app.mode, Mode::Search);
        type_str(&mut app, "dead");
        press(&mut app, KeyCode::Enter);

        // Anywhere in the box, with a kept search: editing it, like `/`.
        let buf = buffer(&mut app, 110, 24);
        let (x, y) = find(&buf, "(esc to clear)");
        click(&mut app, (x + 30, y));
        assert_eq!(app.mode, Mode::Search);
        assert_eq!(app.query, "dead");
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.query, "dea");
    }

    #[test]
    fn mouse_does_nothing_in_search_mode() {
        let mut app = App::new();
        let feed = buffer(&mut app, 110, 24);
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "e");
        let buf = buffer(&mut app, 110, 24);
        let view = app.view.clone();
        let title = title_of(&app, 1);
        let targets = [
            find(&buf, &title),
            find_all(&buf, "▲")[0],
            *find_all(&buf, "☆ Save").last().expect("a card is drawn"),
            find(&buf, "3 Top"),
            find(&feed, "↗ Popular"),
            find(&buf, "⌕"),
        ];
        for at in targets {
            for kind in [
                MouseEventKind::Down(MouseButton::Left),
                MouseEventKind::ScrollDown,
                MouseEventKind::ScrollUp,
            ] {
                mouse(&mut app, kind, at);
                assert_eq!(app.mode, Mode::Search, "{kind:?} at {at:?}");
                assert_eq!(app.query, "e");
                assert_eq!((app.view.clone(), app.selected), (view.clone(), 0));
                assert_eq!((app.feed, app.sort), (Feed::Home, Sort::Hot));
                buffer(&mut app, 110, 24);
            }
        }
        assert!(app.posts.iter().all(|p| p.vote == Vote::None && !p.saved));
    }

    #[test]
    fn only_left_presses_and_the_wheel_clear_the_toast() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('c'));
        let buf = buffer(&mut app, 110, 24);
        let title = title_of(&app, 1);
        let card = find(&buf, &title);
        let save = *find_all(&buf, "☆ Save").last().expect("card 1 is drawn");
        for (kind, at) in [
            (MouseEventKind::Moved, card),
            (MouseEventKind::Up(MouseButton::Left), card),
            (MouseEventKind::Drag(MouseButton::Left), card),
            (MouseEventKind::Down(MouseButton::Right), card),
            (MouseEventKind::Down(MouseButton::Middle), card),
            (MouseEventKind::Down(MouseButton::Right), save),
            (MouseEventKind::Up(MouseButton::Right), save),
        ] {
            mouse(&mut app, kind, at);
            assert_eq!(app.status, Some("Link copied to clipboard!"), "{kind:?}");
            assert_eq!((app.mode, app.selected), (Mode::Feed, 0), "{kind:?}");
            assert!(!app.posts[app.view[1]].saved);
        }

        // A left click on nothing (the logo) clears it and does nothing else.
        click(&mut app, find(&buf, "workit"));
        assert_eq!(app.status, None);
        assert_eq!((app.mode, app.selected), (Mode::Feed, 0));

        press(&mut app, KeyCode::Char('c'));
        buffer(&mut app, 110, 24);
        mouse(&mut app, MouseEventKind::ScrollDown, (100, 1));
        assert_eq!(app.status, None, "the wheel clears it too");
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn wheel_moves_the_feed_selection_and_is_clamped() {
        let mut app = App::new();
        let last = app.view.len() - 1;
        let wheel = |app: &mut App, kind, at| {
            buffer(app, 110, 24);
            mouse(app, kind, at);
            app.selected
        };
        // Wherever the pointer is: a card, the nav, the sidebar, the footer.
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollDown, (30, 8)), 1);
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollDown, (5, 15)), 2);
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollUp, (90, 12)), 1);
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollUp, (40, 23)), 0);
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollUp, (30, 8)), 0);
        for _ in 0..last + 5 {
            wheel(&mut app, MouseEventKind::ScrollDown, (30, 8));
        }
        assert_eq!(app.selected, last);
        assert_eq!(app.mode, Mode::Feed);
        buffer(&mut app, 110, 24);
        assert!(app.offset > 0, "the renderer follows the selection");
    }

    #[test]
    fn wheel_scrolls_the_post_view_three_lines_and_is_clamped() {
        let mut app = thread_post();
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('G'));
        buffer(&mut app, 110, 16);
        let max = scroll(&app);
        assert!(max > 9, "{max}");
        press(&mut app, KeyCode::Char('g'));
        buffer(&mut app, 110, 16);

        let wheel = |app: &mut App, kind| {
            mouse(app, kind, (40, 8));
            buffer(app, 110, 16);
            scroll(app)
        };
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollDown), 3);
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollDown), 6);
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollUp), 3);
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollUp), 0);
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollUp), 0, "top");
        for _ in 0..20 {
            wheel(&mut app, MouseEventKind::ScrollDown);
        }
        assert_eq!(scroll(&app), max, "bottom");
        assert_eq!(wheel(&mut app, MouseEventKind::ScrollUp), max - 3);
        assert_eq!(app.thread, Default::default(), "no focus, no collapse");
    }

    #[test]
    fn clicking_a_comment_header_focuses_it_without_scrolling() {
        let mut app = thread_post();
        let feed = buffer(&mut app, 110, 16);
        press(&mut app, KeyCode::Enter);
        buffer(&mut app, 110, 16);
        for _ in 0..3 {
            mouse(&mut app, MouseEventKind::ScrollDown, (40, 8));
            buffer(&mut app, 110, 16);
        }
        assert_eq!(scroll(&app), 9);
        let buf = buffer(&mut app, 110, 16);
        let s = rows(&buf).join("\n");

        // Bodies, action rows, guides below the header, the post, the nav,
        // the search box and where the tabs were do nothing.
        let (bx, by) = find(&buf, "bob replies");
        let (_, ry) = find_all(&buf, "Reply")[0];
        for at in [
            (bx, by),
            (bx, ry),
            (bx - 2, by),
            find(&feed, "⌕"),
            find(&feed, "↗ Popular"),
            find(&feed, "3 Top"),
            find(&feed, "Thread title"),
        ] {
            click(&mut app, at);
            assert_eq!(app.thread, Default::default(), "{at:?}:\n{s}");
            assert_eq!(app.mode, Mode::Post { scroll: 9 });
            buffer(&mut app, 110, 16);
        }

        // bob's header: on the name, or anywhere else on its row.
        let (x, y) = find(&buf, "u/bob · 1h ago");
        click(&mut app, (x + 30, y));
        assert_eq!(app.thread.focus, Some(vec![0, 0]));
        assert!(!app.thread.reveal, "a click never asks for a scroll");
        assert_eq!(app.mode, Mode::Post { scroll: 9 });
        let buf = buffer(&mut app, 110, 16);
        assert_eq!(scroll(&app), 9);
        assert!(is_focus_style(&buf, x, y));

        // A header on the last row of the post view, with its body below
        // the edge.
        let (x, y) = find(&buf, "u/cat · 1h ago");
        assert_eq!(y, 13, "the last row of the post view:\n{s}");
        click(&mut app, (x, y));
        assert_eq!(app.thread.focus, Some(vec![0, 0, 0]));
        buffer(&mut app, 110, 16);
        assert_eq!(scroll(&app), 9, "not scrolled to show cat's body");
    }

    #[test]
    fn clicking_the_focused_header_collapses_and_expands_it() {
        let mut app = thread_post();
        press(&mut app, KeyCode::Enter);
        let buf = buffer(&mut app, 110, 40);
        let (x, y) = find(&buf, "u/ann · 1h ago");
        click(&mut app, (x, y));
        assert_eq!(app.thread.focus, Some(vec![0]));
        assert!(
            app.thread.collapsed.is_empty(),
            "the first click only focuses"
        );

        buffer(&mut app, 110, 40);
        click(&mut app, (x + 5, y));
        let buf = buffer(&mut app, 110, 40);
        let header = find(&buf, "[+] u/ann · 1h ago (2 replies)");
        assert_eq!(header, (x, y));
        assert_eq!(app.thread.focus, Some(vec![0]));

        // The `[+]` itself is part of the header.
        click(&mut app, header);
        let buf = buffer(&mut app, 110, 40);
        assert!(app.thread.collapsed.is_empty());
        assert!(find_all(&buf, "[+]").is_empty());
        assert_eq!(find(&buf, "ann says hi").1, y + 1);

        // A collapsed header that isn't focused is focused, not expanded.
        let (dx, dy) = find(&buf, "u/dan · 1h ago");
        click(&mut app, (dx, dy));
        buffer(&mut app, 110, 40);
        click(&mut app, (dx, dy));
        let buf = buffer(&mut app, 110, 40);
        let dan = find(&buf, "[+] u/dan · 1h ago (1 reply)");
        click(&mut app, (x, y));
        assert_eq!(app.thread.focus, Some(vec![0]));
        buffer(&mut app, 110, 40);
        click(&mut app, dan);
        assert_eq!(app.thread.focus, Some(vec![1]));
        assert!(app.thread.collapsed.contains(&vec![1]), "still collapsed");
        buffer(&mut app, 110, 40);
        click(&mut app, dan);
        assert!(app.thread.collapsed.is_empty());
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
    }

    #[test]
    fn mouse_works_in_an_empty_feed() {
        let mut app = App::new();
        type_str(&mut app, "]]]");
        assert_eq!(app.feed, Feed::Saved);
        assert!(app.view.is_empty());
        let buf = buffer(&mut app, 110, 24);
        let (x, y) = find(&buf, "No saved posts yet");
        click(&mut app, (x, y));
        mouse(&mut app, MouseEventKind::ScrollDown, (x, y));
        assert_eq!((app.mode, app.selected), (Mode::Feed, 0));

        click(&mut app, find(&buf, "3 Top"));
        assert_eq!(app.sort, Sort::Top);
        buffer(&mut app, 110, 24);
        click(&mut app, find(&buf, "Search Saved"));
        assert_eq!(app.mode, Mode::Search);
        press(&mut app, KeyCode::Esc);
        buffer(&mut app, 110, 24);
        click(&mut app, find(&buf, "◎ All"));
        assert_eq!(app.feed, Feed::All);
        assert!(!app.view.is_empty());
    }

    /// Every field of `app`, so that "changes nothing" means nothing.
    fn state(app: &App) -> String {
        format!(
            "{:?}",
            (
                (&app.groups, &app.posts, &app.view, app.selected, app.offset),
                (app.sort, app.feed, app.mode, &app.query, &app.search),
                (
                    &app.thread,
                    &app.hits,
                    app.status,
                    app.karma,
                    app.should_quit
                ),
            )
        )
    }

    #[test]
    fn mouse_events_the_app_ignores_change_nothing() {
        let kinds = [
            MouseEventKind::Moved,
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::Down(MouseButton::Right),
            MouseEventKind::Up(MouseButton::Right),
            MouseEventKind::Drag(MouseButton::Right),
            MouseEventKind::Down(MouseButton::Middle),
            MouseEventKind::ScrollDown,
            MouseEventKind::ScrollUp,
            MouseEventKind::ScrollLeft,
            MouseEventKind::ScrollRight,
        ];
        let counts = |kind: MouseEventKind| {
            matches!(
                kind,
                MouseEventKind::Down(MouseButton::Left)
                    | MouseEventKind::ScrollDown
                    | MouseEventKind::ScrollUp
            )
        };
        // (setup keys, whether the app should handle anything).
        let setups: [(&str, bool); 4] = [("", true), ("/e", false), ("\nn", true), ("", false)];
        for (n, (keys, handles)) in setups.into_iter().enumerate() {
            let mut app = thread_post();
            press(&mut app, KeyCode::Char('c'));
            for ch in keys.chars() {
                press(
                    &mut app,
                    if ch == '\n' {
                        KeyCode::Enter
                    } else {
                        KeyCode::Char(ch)
                    },
                );
            }
            // The last setup is before the first frame.
            let buf = (n < 3).then(|| buffer(&mut app, 110, 24));
            assert_eq!(app.hits.is_some(), n < 3);
            let title = buf.map_or((40, 8), |buf| find(&buf, "Thread"));
            for at in [title, (5, 5), (40, 8), (60, 1), (0, 23)] {
                for kind in kinds {
                    let event = MouseEvent {
                        kind,
                        column: at.0,
                        row: at.1,
                        modifiers: KeyModifiers::NONE,
                    };
                    let expected = handles && counts(kind);
                    assert_eq!(app.handles_mouse(&event), expected, "{n}: {kind:?}");
                    if !expected {
                        let before = state(&app);
                        app.on_mouse(event);
                        assert_eq!(state(&app), before, "{n}: {kind:?} at {at:?}");
                    }
                }
            }
        }
    }
}
