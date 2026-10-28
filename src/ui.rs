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
use crate::app::Feed;
use crate::app::Mode;
use crate::app::Query;
use crate::app::Sort;
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

pub(crate) fn render(frame: &mut Frame, app: &mut App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    render_header(frame, app, header);
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
        render_nav(frame, app, nav);
    }
    if show_sidebar {
        render_sidebar(frame, app, sidebar);
    }
    match app.mode {
        Mode::Post { scroll } => render_post(frame, app, main, scroll),
        Mode::Feed | Mode::Search => render_feed(frame, app, main),
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
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
        Mode::Post { .. } => &[
            ("j/k", "scroll"),
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

fn render_nav(frame: &mut Frame, app: &App, area: Rect) {
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

    let lines: Vec<Line> = [
        item("⌂ Home".to_owned(), Feed::Home),
        item("↗ Popular".to_owned(), Feed::Popular),
        item("◎ All".to_owned(), Feed::All),
        item("☆ Saved".to_owned(), Feed::Saved),
        Line::raw(""),
        Line::styled(" GROUPS", Style::new().fg(DIM)),
    ]
    .into_iter()
    .chain(app.groups.iter().enumerate().map(|(i, g)| {
        let mark = if g.joined { "●" } else { "○" };
        item(
            truncate(&format!("{mark} w/{}", g.name), usize::from(NAV_WIDTH) - 3),
            Feed::Group(i),
        )
    }))
    .collect();

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(BORDER),
        ),
        area,
    );
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

fn render_feed(frame: &mut Frame, app: &mut App, area: Rect) {
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
    frame.render_widget(
        Paragraph::new(Line::from(tab_spans)).block(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(BORDER)
                .title(Span::styled(
                    format!(" {} ", app.feed_name()),
                    Style::new().fg(Color::White).bold(),
                ))
                .title_bottom(
                    Line::styled(format!(" {} posts ", app.view.len()), Style::new().fg(DIM))
                        .right_aligned(),
                ),
        ),
        tabs,
    );

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
        render_card(frame, app, &app.posts[pi], rect, vi == app.selected);
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

fn actions_line(post: &Post) -> Line<'static> {
    let dim = Style::new().fg(DIM).bold();
    let save = if post.saved {
        Span::styled("★ Saved", Style::new().fg(ORANGE).bold())
    } else {
        Span::styled("☆ Save", dim)
    };
    Line::from(vec![
        Span::styled(
            format!("▤ {} Comments", compact(i64::from(post.comment_count))),
            dim,
        ),
        Span::raw("   "),
        Span::styled("↗ Share", dim),
        Span::raw("   "),
        save,
        Span::raw("   "),
        Span::styled("···", dim),
    ])
}

fn render_card(frame: &mut Frame, app: &App, post: &Post, area: Rect, selected: bool) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(if selected { ORANGE } else { BORDER });
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [votes, content] =
        Layout::horizontal([Constraint::Length(VOTE_WIDTH), Constraint::Min(0)]).areas(inner);

    let color = vote_color(post.vote);
    let arrow = |active: bool, glyph: &'static str, c: Color| {
        Span::styled(glyph, Style::new().fg(if active { c } else { DIM }).bold())
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(arrow(post.vote == Vote::Up, "▲", ORANGE)),
            Line::styled(
                compact(i64::from(post.score)),
                Style::new().fg(color).bold(),
            ),
            Line::from(arrow(post.vote == Vote::Down, "▼", PERIWINKLE)),
        ])
        .centered()
        .bg(if selected { HIGHLIGHT_BG } else { Color::Reset }),
        votes,
    );

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
        .chain(std::iter::once(actions_line(post)))
        .collect();
    frame.render_widget(
        Paragraph::new(lines),
        Rect {
            x: content.x + 1,
            width: content.width.saturating_sub(1),
            ..content
        },
    );
}

fn comment_lines(comment: &Comment, depth: usize, width: usize, out: &mut Vec<Line<'static>>) {
    let guide = Style::new().fg(BORDER);
    let prefix = "│ ".repeat(depth);
    let text_width = width.saturating_sub(prefix.chars().count());

    out.push(Line::from(vec![
        Span::styled(prefix.clone(), guide),
        Span::styled(format!("u/{}", comment.author), Style::new().bold()),
        Span::styled(
            format!(" · {}", age(comment.hours_ago)),
            Style::new().fg(DIM),
        ),
    ]));
    out.extend(
        wrap(comment.body, text_width)
            .into_iter()
            .map(|l| Line::from(vec![Span::styled(prefix.clone(), guide), Span::raw(l)])),
    );
    out.push(Line::from(vec![
        Span::styled(prefix.clone(), guide),
        Span::styled(
            format!("▲ {} ▼   Reply   Share", compact(i64::from(comment.score))),
            Style::new().fg(DIM).bold(),
        ),
    ]));
    out.push(Line::styled(format!("{prefix}│"), guide));
    comment
        .replies
        .iter()
        .for_each(|r| comment_lines(r, depth + 1, width, out));
}

fn render_post(frame: &mut Frame, app: &mut App, area: Rect, scroll: u16) {
    let Some(post) = app.selected_post() else {
        return;
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(ORANGE);
    let inner = block.inner(area);
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
    let mut action = actions_line(post);
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

    let mut comments = Vec::new();
    post.comments
        .iter()
        .for_each(|c| comment_lines(c, 0, width, &mut comments));
    lines.extend(comments);
    let shown: u32 = post.comments.iter().map(Comment::total).sum();
    let hidden = post.comment_count.saturating_sub(shown);
    if hidden > 0 {
        lines.push(Line::styled(
            format!("··· {hidden} more comments"),
            Style::new().fg(PERIWINKLE).bold(),
        ));
    }

    let max_scroll = (lines.len() as u16).saturating_sub(inner.height);
    let scroll = scroll.min(max_scroll);
    app.mode = Mode::Post { scroll };

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

    use super::*;

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
            press(&mut app, KeyCode::Esc);

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
}
