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
use crate::app::Drawn;
use crate::app::Feed;
use crate::app::Mode;
use crate::app::Sort;
use crate::app::Thread;
use crate::app::drawn_comments;
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
        Mode::Search => &[("type", "filter"), ("enter", "keep"), ("esc", "clear")],
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
        let (headline, hint) = if !app.query.is_empty() {
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

fn preview(post: &Post, width: usize) -> Vec<String> {
    let flat = post.body.replace('\n', " ");
    let mut lines = wrap(&flat, width);
    if lines.len() > PREVIEW_LINES {
        lines.truncate(PREVIEW_LINES);
        let last = lines.last_mut().expect("should have PREVIEW_LINES lines");
        *last = truncate(&format!("{last} …"), width);
    }
    lines
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
        .chain(
            wrap(post.title, width)
                .into_iter()
                .map(|l| Line::styled(l, title_style)),
        )
        .chain(
            preview(post, width)
                .into_iter()
                .map(|l| Line::styled(l, Style::new().fg(Color::Gray))),
        )
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

/// Pushes the rows of one drawn comment. Its replies are drawn separately.
fn comment_lines(drawn: &Drawn, width: usize, thread: &Thread, out: &mut Vec<Line<'static>>) {
    let comment = drawn.comment;
    let guide = Style::new().fg(BORDER);
    let prefix = "│ ".repeat(drawn.depth());
    let text_width = width.saturating_sub(prefix.chars().count());
    let collapsed = thread.collapsed.contains(&drawn.id);
    let focused = thread.focus == Some(drawn.id);

    let (marker, suffix) = match collapsed.then(|| comment.total() - 1) {
        None => ("", String::new()),
        Some(0) => ("[+] ", String::new()),
        Some(1) => ("[+] ", " (1 reply)".to_owned()),
        Some(n) => ("[+] ", format!(" ({n} replies)")),
    };
    let header = if focused {
        let text = format!(
            "{marker}u/{} · {}{suffix}",
            comment.author,
            age(comment.hours_ago)
        );
        vec![Span::styled(
            format!("{text:<text_width$}"),
            Style::new().fg(ORANGE).bg(HIGHLIGHT_BG).bold(),
        )]
    } else {
        vec![
            Span::styled(marker, Style::new().fg(DIM)),
            Span::styled(format!("u/{}", comment.author), Style::new().bold()),
            Span::styled(
                format!(" · {}{suffix}", age(comment.hours_ago)),
                Style::new().fg(DIM),
            ),
        ]
    };
    out.push(Line::from(
        [Span::styled(prefix.clone(), guide)]
            .into_iter()
            .chain(header)
            .collect::<Vec<_>>(),
    ));
    if collapsed {
        out.push(Line::styled(format!("{prefix}│"), guide));
        return;
    }

    out.extend(
        wrap(comment.body, text_width)
            .into_iter()
            .map(|l| Line::from(vec![Span::styled(prefix.clone(), guide), Span::raw(l)])),
    );
    let arrow = |active: bool, glyph: &'static str, c: Color| {
        Span::styled(glyph, Style::new().fg(if active { c } else { DIM }).bold())
    };
    let score_color = match comment.vote {
        Vote::None => DIM,
        vote => vote_color(vote),
    };
    out.push(Line::from(vec![
        Span::styled(prefix.clone(), guide),
        arrow(comment.vote == Vote::Up, "▲", ORANGE),
        Span::styled(
            format!(" {} ", compact(i64::from(comment.score))),
            Style::new().fg(score_color).bold(),
        ),
        arrow(comment.vote == Vote::Down, "▼", PERIWINKLE),
        Span::styled("   Reply   Share", Style::new().fg(DIM).bold()),
    ]));
    out.push(Line::styled(format!("{prefix}│"), guide));
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
    lines.extend(
        wrap(post.title, width)
            .into_iter()
            .map(|l| Line::styled(l, Style::new().fg(Color::White).bold())),
    );
    lines.push(Line::raw(""));
    lines.extend(wrap(post.body, width).into_iter().map(Line::raw));
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

    let mut focus_row = None;
    for drawn in drawn_comments(&post.comments, &app.thread.collapsed) {
        if app.thread.focus == Some(drawn.id) {
            focus_row = Some(lines.len());
        }
        comment_lines(&drawn, width, &app.thread, &mut lines);
    }
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
    // Right after the focus moves, scroll by the least amount that brings the
    // focused comment's header into view.
    if app.thread.reveal
        && let Some(row) = focus_row
    {
        let row = row as u16;
        scroll = scroll.clamp(row.saturating_sub(inner.height.saturating_sub(1)), row);
    }
    app.thread.reveal = false;
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
    use ratatui::crossterm::event::KeyCode;
    use ratatui::crossterm::event::KeyEvent;
    use ratatui::crossterm::event::KeyModifiers;

    use super::*;

    fn screen(app: &mut App, width: u16, height: u16) -> String {
        let mut terminal =
            Terminal::new(TestBackend::new(width, height)).expect("test backend is infallible");
        terminal
            .draw(|f| render(f, app))
            .expect("test backend is infallible");
        let buf = terminal.backend().buffer();
        (0..height)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
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
            assert!(post.contains("n/p comments"), "{feed:?}: {post:?}");
            assert!(
                !app.selected_post()
                    .expect("just opened")
                    .comments
                    .is_empty(),
                "{feed:?} needs a comment to focus"
            );
            press(&mut app, KeyCode::Char('n'));
            let focused = footer(&mut app);
            assert!(
                focused.contains("a/z vote comment") && focused.ends_with("esc unfocus"),
                "{feed:?}: {focused:?}"
            );
            press(&mut app, KeyCode::Esc);
            press(&mut app, KeyCode::Esc);

            press(&mut app, KeyCode::Char(']'));
        }
        assert_eq!(app.feed, Feed::Home, "visited every feed");
    }

    fn comment(author: &'static str, body: &'static str, replies: Vec<Comment>) -> Comment {
        Comment {
            author,
            body,
            score: 10,
            hours_ago: 1,
            replies,
            vote: Vote::None,
        }
    }

    /// Opens the first post on Home with its comments replaced by `comments`.
    fn open_with(comments: Vec<Comment>) -> App {
        let mut app = App::new();
        let i = app.view[0];
        app.posts[i].comments = comments;
        app.posts[i].comment_count = 50;
        press(&mut app, KeyCode::Enter);
        app
    }

    /// alpha > bravo > charlie, delta > echo, and foxtrot: 2, 1 and 0 replies.
    fn three_threads() -> App {
        open_with(vec![
            comment(
                "alpha",
                "alpha body",
                vec![comment(
                    "bravo",
                    "bravo body",
                    vec![comment("charlie", "charlie body", vec![])],
                )],
            ),
            comment(
                "delta",
                "delta body",
                vec![comment("echo", "echo body", vec![])],
            ),
            comment("foxtrot", "foxtrot body", vec![]),
        ])
    }

    fn row_of(s: &str, needle: &str) -> usize {
        s.lines()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("missing {needle:?} in:\n{s}"))
    }

    #[test]
    fn collapsed_comment_shows_reply_count_and_hides_its_thread() {
        let mut app = three_threads();
        let s = screen(&mut app, 140, 60);
        let bodies = ["alpha body", "bravo", "charlie", "delta body", "echo"];
        for needle in bodies {
            assert!(
                s.contains(needle),
                "missing {needle:?} before collapsing:\n{s}"
            );
        }
        assert!(!s.contains("[+]") && !s.contains("[–]"), "{s}");

        // n, Enter on each top-level comment collapses all three.
        for _ in 0..3 {
            press(&mut app, KeyCode::Char('n'));
            press(&mut app, KeyCode::Enter);
        }
        let s = screen(&mut app, 140, 60);
        assert!(s.contains("[+] u/alpha · 1h ago (2 replies)"), "{s}");
        assert!(s.contains("[+] u/delta · 1h ago (1 reply)"), "{s}");
        let foxtrot = s
            .split("[+] u/foxtrot")
            .nth(1)
            .and_then(|rest| rest.split('│').next())
            .expect("found");
        assert_eq!(foxtrot.trim_end(), " · 1h ago", "no suffix without replies");
        for needle in bodies.into_iter().chain(["foxtrot body", "Reply"]) {
            assert!(!s.contains(needle), "{needle:?} should be hidden:\n{s}");
        }
        assert_eq!(
            row_of(&s, "u/delta"),
            row_of(&s, "u/alpha") + 2,
            "a collapsed comment is its header and its guide row"
        );
    }

    /// Renders `app` and returns the cell where `needle` starts.
    fn cell_at(app: &mut App, needle: &str) -> ratatui::buffer::Cell {
        let mut terminal =
            Terminal::new(TestBackend::new(140, 60)).expect("test backend is infallible");
        terminal
            .draw(|f| render(f, app))
            .expect("test backend is infallible");
        let buf = terminal.backend().buffer();
        for y in 0..buf.area.height {
            let row: Vec<&str> = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
            if let Some(x) = (0..row.len()).find(|&x| row[x..].concat().starts_with(needle)) {
                return buf[(x as u16, y)].clone();
            }
        }
        panic!("missing {needle:?}");
    }

    #[test]
    fn focused_comment_header_uses_the_nav_highlight_and_votes_are_colored() {
        let mut app = three_threads();
        press(&mut app, KeyCode::Char('n'));
        let cell = cell_at(&mut app, "u/alpha");
        assert_eq!((cell.fg, cell.bg), (ORANGE, HIGHLIGHT_BG));
        assert!(cell.modifier.contains(Modifier::BOLD));
        assert_ne!(cell_at(&mut app, "u/bravo").bg, HIGHLIGHT_BG);

        press(&mut app, KeyCode::Char('a'));
        assert_eq!(cell_at(&mut app, "▲ 11 ▼").fg, ORANGE, "upvoted arrow");
        assert_eq!(cell_at(&mut app, "11 ▼").fg, ORANGE, "upvoted score");
        assert_eq!(cell_at(&mut app, "▼   Reply").fg, DIM);
        assert_eq!(cell_at(&mut app, "▲ 10 ▼").fg, DIM, "no vote stays dim");
        assert_eq!(cell_at(&mut app, "10 ▼").fg, DIM);

        press(&mut app, KeyCode::Char('z'));
        assert_eq!(cell_at(&mut app, "▲ 9 ▼").fg, DIM);
        assert_eq!(cell_at(&mut app, "9 ▼").fg, PERIWINKLE, "downvoted score");
        assert_eq!(cell_at(&mut app, "▼   Reply").fg, PERIWINKLE);
    }

    #[test]
    fn comment_scores_stay_compacted() {
        let mut app = three_threads();
        let i = app.view[0];
        app.posts[i].comments[0].score = 1_242;
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(cell_at(&mut app, "▲ 1.2k ▼   Reply").fg, ORANGE);
        assert_eq!(app.posts[i].comments[0].score, 1_243);
    }

    #[test]
    fn more_comments_count_ignores_collapse() {
        let mut app = three_threads();
        let s = screen(&mut app, 140, 60);
        assert!(
            s.contains("··· 44 more comments"),
            "50 minus 6 seeded:\n{s}"
        );
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Enter);
        let s = screen(&mut app, 140, 60);
        assert!(s.contains("[+] u/alpha"), "{s}");
        assert!(s.contains("··· 44 more comments"), "{s}");
    }

    #[test]
    fn collapsing_keeps_scroll_clamped_to_the_shorter_content() {
        let mut app = three_threads();
        // echo, the fifth comment, is on screen at the bottom of the post.
        (0..5).for_each(|_| press(&mut app, KeyCode::Char('n')));
        screen(&mut app, 110, 16);
        press(&mut app, KeyCode::Char('G'));
        let s = screen(&mut app, 110, 16);
        assert!(s.contains("u/echo"), "{s}");
        let Mode::Post { scroll: before } = app.mode else {
            panic!("should still be in post view");
        };
        press(&mut app, KeyCode::Enter);
        let s = screen(&mut app, 110, 16);
        assert!(s.contains("[+] u/echo"), "{s}");
        assert_eq!(
            app.mode,
            Mode::Post { scroll: before - 2 },
            "its body and action row are gone, so the bottom is 2 rows higher"
        );
        press(&mut app, KeyCode::Char('j'));
        screen(&mut app, 110, 16);
        assert_eq!(
            app.mode,
            Mode::Post { scroll: before - 2 },
            "already at the bottom"
        );
        press(&mut app, KeyCode::Char('k'));
        screen(&mut app, 110, 16);
        assert_eq!(
            app.mode,
            Mode::Post { scroll: before - 3 },
            "k moves right away"
        );
    }

    #[test]
    fn focusing_a_comment_off_screen_scrolls_it_into_view_once() {
        const AUTHORS: [&str; 8] = [
            "user0", "user1", "user2", "user3", "user4", "user5", "user6", "user7",
        ];
        let mut app = open_with(
            AUTHORS
                .iter()
                .map(|&a| comment(a, "short", vec![]))
                .collect(),
        );
        // At 16 rows the post box spans rows 3..=14, so rows 4..=13 are content.
        let (width, height) = (110, 16);
        let (top, bottom) = (4, height as usize - 3);
        let scroll = |app: &App| match app.mode {
            Mode::Post { scroll } => scroll,
            _ => panic!("should still be in post view"),
        };
        let mut s = screen(&mut app, width, height);
        assert!(!s.contains("u/user7"), "the last comment starts off screen");

        for author in AUTHORS {
            press(&mut app, KeyCode::Char('n'));
            s = screen(&mut app, width, height);
            let row = row_of(&s, &format!("u/{author}"));
            assert!((top..=bottom).contains(&row), "{author} at {row}:\n{s}");
        }
        assert_eq!(
            row_of(&s, "u/user7"),
            bottom,
            "scrolled by the least amount, so the header is on the last row"
        );
        let revealed = scroll(&app);
        assert!(revealed > 0);

        press(&mut app, KeyCode::Char('k'));
        s = screen(&mut app, width, height);
        assert_eq!(scroll(&app), revealed - 1, "k isn't pulled back:\n{s}");
        assert!(!s.contains("u/user7"));
        s = screen(&mut app, width, height);
        assert_eq!(scroll(&app), revealed - 1, "nor on the next frame");

        // Upwards: from the bottom, p back to the top comment.
        press(&mut app, KeyCode::Char('G'));
        screen(&mut app, width, height);
        for _ in 0..AUTHORS.len() {
            press(&mut app, KeyCode::Char('p'));
            s = screen(&mut app, width, height);
        }
        assert_eq!(row_of(&s, "u/user0"), top, "header on the first row:\n{s}");
    }

    #[test]
    fn voting_or_collapsing_an_off_screen_focus_scrolls_it_back_into_view() {
        let (width, height) = (110, 16);
        let (top, bottom) = (4, height as usize - 3);
        let scroll = |app: &App| match app.mode {
            Mode::Post { scroll } => scroll,
            _ => panic!("should still be in post view"),
        };
        for (key, done) in [
            (
                KeyCode::Char('a'),
                (|app: &App| app.selected_post().expect("open").comments[0].vote == Vote::Up)
                    as fn(&App) -> bool,
            ),
            (KeyCode::Enter, |app: &App| {
                app.thread.collapsed.contains(&0)
            }),
        ] {
            let mut app = three_threads();
            press(&mut app, KeyCode::Char('n'));
            screen(&mut app, width, height);
            press(&mut app, KeyCode::Char('G'));
            let s = screen(&mut app, width, height);
            assert!(!s.contains("u/alpha"), "{key:?}: scrolled off screen:\n{s}");
            let off = scroll(&app);

            press(&mut app, key);
            assert!(done(&app), "{key:?} acted on alpha");
            let s = screen(&mut app, width, height);
            assert_eq!(
                row_of(&s, "u/alpha"),
                top,
                "{key:?}: least scroll up puts the header on the first row:\n{s}"
            );
            let revealed = scroll(&app);
            assert!(revealed < off);

            press(&mut app, KeyCode::Char('j'));
            screen(&mut app, width, height);
            assert_eq!(scroll(&app), revealed + 1, "{key:?}: j isn't pulled back");
            let s = screen(&mut app, width, height);
            assert_eq!(scroll(&app), revealed + 1, "{key:?}: nor on the next frame");
            assert!((top..=bottom).all(|r| !s.lines().nth(r).expect("row").contains("u/alpha")));
        }
    }
}
