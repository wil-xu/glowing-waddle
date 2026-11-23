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
use ratatui::widgets::Clear;
use ratatui::widgets::Padding;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::app::App;
use crate::app::CommentPath;
use crate::app::Density;
use crate::app::Feed;
use crate::app::Help;
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
/// A compact row's score column: wide enough for every seed score, so the
/// arrows line up from row to row.
const SCORE_WIDTH: usize = 5;
/// The room a compact row keeps for its title by shortening its meta.
const MIN_TITLE: usize = 20;

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
    // Last, so it covers the body as drawn. Header and footer stay.
    render_help(frame, app, body, &mut hits);
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

/// A footer hint: a key, then what it does.
type Hint = (&'static str, &'static str);

// Every hint set must fit on one line at 80 columns, counting the blank after
// the last hint, on every feed, in both views.
//
// The feed sets leave out `j/k`, `c` (which the post view set shows) and
// `Tab` (only `1-4`). With `J join` the group set would be 84 with
// `enter open`, and with `v view` the plain set would be too, so both leave
// it out: Enter is the most guessable key, and clicking a card or a compact
// row opens it too. Adding `? help` made the group set 88, so both sets now
// give up `v view` for it: the view switch (`▦ Card` / `≡ Compact`) on the tab
// row already names the view and toggles it on a click, and `?` lists `v`
// along with every other key the footer can't fit. That leaves 72 and 80, and
// the group set is still the plain set plus `J join`.
const FEED_HINTS: &[Hint] = &[
    ("a/z", "vote"),
    ("1-4", "sort"),
    ("[/]", "feed"),
    ("/", "search"),
    ("s", "save"),
    ("h", "hide"),
    ("?", "help"),
    ("q", "quit"),
];
const GROUP_HINTS: &[Hint] = &[
    ("a/z", "vote"),
    ("1-4", "sort"),
    ("[/]", "feed"),
    ("/", "search"),
    ("s", "save"),
    ("h", "hide"),
    ("J", "join"),
    ("?", "help"),
    ("q", "quit"),
];
const KEPT_SEARCH_HINTS: &[Hint] = &[
    ("j/k", "move"),
    ("enter", "open"),
    ("/", "edit search"),
    ("esc", "clear search"),
    ("q", "quit"),
];
const SEARCH_HINTS: &[Hint] = &[
    ("type", "filter"),
    ("w/ u/ flair:", "narrow"),
    ("enter", "keep"),
    ("esc", "clear"),
];
const POST_HINTS: &[Hint] = &[
    ("j/k", "scroll"),
    ("n/p", "comments"),
    ("a/z", "vote"),
    ("s", "save"),
    ("c", "share"),
    ("?", "help"),
    ("esc", "back"),
];
// With `? help` this set would be 81, so it leaves out `j/k scroll`: the set
// above always shows it first, since a comment is only focused from the
// unfocused post view, and the wheel scrolls too.
const FOCUSED_HINTS: &[Hint] = &[
    ("n/p", "comments"),
    ("a/z", "vote comment"),
    ("enter", "collapse"),
    ("?", "help"),
    ("esc", "unfocus"),
];
// Short enough (42 columns) that `close` still shows at 50 columns. The
// help itself lists `PgDn`/`PgUp`/`Space`, on screen while this is.
const HELP_HINTS: &[Hint] = &[("j/k", "scroll"), ("g/G", "top/bottom"), ("?/esc", "close")];

/// The footer's hint set for the current mode.
fn hints(app: &App) -> &'static [Hint] {
    if app.help.is_some() {
        return HELP_HINTS;
    }
    match app.mode {
        Mode::Feed if !app.query.is_empty() => KEPT_SEARCH_HINTS,
        Mode::Feed if matches!(app.feed, Feed::Group(_)) => GROUP_HINTS,
        Mode::Feed => FEED_HINTS,
        Mode::Search => SEARCH_HINTS,
        Mode::Post { .. } if app.thread.focus.is_some() => FOCUSED_HINTS,
        Mode::Post { .. } => POST_HINTS,
    }
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

    let spans: Vec<Span> = hints(app)
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

/// A part of the help overlay, one per mode plus the mouse and the keys that
/// work anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Feed,
    Post,
    Search,
    Mouse,
    Help,
    Anywhere,
}

/// One line of the help: the keys (shown joined by `, `), then what they do.
type HelpRow = (&'static [&'static str], &'static str);

/// Every key binding, the only place the help's text comes from. The footer
/// shows a few of these; a test checks that each footer key is listed here,
/// in the section for its mode. Keep it in step with PLAN.md's "Key
/// bindings" and "Mouse".
const HELP: [(Section, &str, &[HelpRow]); 6] = [
    (
        Section::Feed,
        "Feed",
        &[
            (&["j/k", "↓/↑"], "select the next / previous post"),
            (&["PgDn/PgUp"], "move 5 posts down / up"),
            (&["g/G", "Home/End"], "first / last post"),
            (&["a/z"], "upvote / downvote, again to undo"),
            (&["s"], "save / unsave"),
            (&["h"], "hide / unhide (hidden posts are in Hidden)"),
            (&["J"], "join / leave the group, on a group feed"),
            (&["v"], "switch between the Card and Compact views"),
            (&["c"], "copy the post's link"),
            (&["enter"], "open the post"),
            (&["1-4"], "sort by Hot, New, Top, Rising"),
            (&["Tab"], "next sort"),
            (&["[/]"], "previous / next feed"),
            (&["/"], "search, editing a kept search"),
            (&["esc"], "clear a kept search"),
            (&["?"], "this help"),
            (&["q"], "quit"),
        ],
    ),
    (
        Section::Post,
        "Post view",
        &[
            (&["j/k", "↓/↑"], "scroll 1 line"),
            (&["PgDn/PgUp"], "scroll 10 lines down / up"),
            (&["Space"], "scroll 10 lines down"),
            (&["g/G", "Home/End"], "top / bottom"),
            (&["n/p"], "focus the next / previous comment"),
            (&["a/z"], "vote on the focused comment, or the post"),
            (&["enter"], "collapse / expand the focused comment"),
            (&["s"], "save / unsave the post"),
            (&["c"], "copy the post's link"),
            (&["esc"], "unfocus the comment, or back to the feed"),
            (&["q", "Backspace"], "back to the feed"),
            (&["?"], "this help"),
        ],
    ),
    (
        Section::Search,
        "Search mode",
        &[
            (&["type"], "filter as you type: q, j, ? and digits too"),
            (&["w/ u/ flair:"], "narrow to a group, author or flair"),
            (&["Backspace"], "delete a character"),
            (&["enter"], "keep the search, back to the feed"),
            (&["esc"], "clear the search, back to the feed"),
        ],
    ),
    (
        Section::Mouse,
        "Mouse",
        &[
            (&["click post"], "open it, like enter"),
            (&["click ▲/▼"], "vote on it, like a/z"),
            (&["click Save/☆"], "save / unsave it, like s"),
            (&["click tab"], "sort, like 1-4"),
            (&["click switch"], "switch the view, like v"),
            (&["click nav"], "go to that feed"),
            (&["click search"], "search, like /"),
            (&["click comment"], "focus it, again to collapse / expand"),
            (&["wheel"], "next / previous post, or scroll a post 3 lines"),
        ],
    ),
    (
        Section::Help,
        "This help",
        &[
            (&["j/k", "↓/↑"], "scroll 1 line"),
            (&["PgDn/PgUp"], "scroll 10 lines down / up"),
            (&["Space"], "scroll 10 lines down"),
            (&["g/G", "Home/End"], "top / bottom"),
            (&["?/esc", "q"], "close"),
            (&["wheel"], "scroll 3 lines"),
            (&["click outside"], "close"),
        ],
    ),
    (
        Section::Anywhere,
        "Anywhere",
        &[(&["Ctrl-C"], "quit, saving votes, saves, hides and groups")],
    ),
];

/// Rows are indented this much under their section's heading.
const HELP_INDENT: usize = 2;
/// Blank columns between the key column and the description.
const HELP_GAP: usize = 2;

/// The width of a section's key column: its longest keys.
fn help_key_width(rows: &[HelpRow]) -> usize {
    rows.iter()
        .map(|(keys, _)| keys.join(", ").chars().count())
        .max()
        .unwrap_or(0)
}

/// The width of the widest help line, unwrapped. Every glyph in `HELP` is one
/// cell wide (a test checks it), so chars are columns.
fn help_width() -> usize {
    HELP.iter()
        .flat_map(|(_, title, rows)| {
            let keys = help_key_width(rows);
            let descriptions = rows
                .iter()
                .map(move |(_, what)| HELP_INDENT + keys + HELP_GAP + what.chars().count());
            std::iter::once(title.chars().count()).chain(descriptions)
        })
        .max()
        .unwrap_or(0)
}

/// The help's lines at `width` columns: the section for the current mode
/// first, then the others in table order, a blank line between sections.
/// Descriptions too long for the line wrap under themselves.
fn help_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    let first = match app.mode {
        Mode::Feed => Section::Feed,
        Mode::Search => Section::Search,
        Mode::Post { .. } => Section::Post,
    };
    let sections = HELP
        .iter()
        .filter(|(section, ..)| *section == first)
        .chain(HELP.iter().filter(|(section, ..)| *section != first));
    let mut lines = Vec::new();
    for (n, (_, title, rows)) in sections.enumerate() {
        if n > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::styled(*title, Style::new().fg(Color::White).bold()));
        let column = HELP_INDENT + help_key_width(rows) + HELP_GAP;
        for (keys, what) in rows.iter() {
            let keys = format!("{}{}", " ".repeat(HELP_INDENT), keys.join(", "));
            for (i, row) in wrap(what, width.saturating_sub(column))
                .into_iter()
                .enumerate()
            {
                let key = if i == 0 { keys.as_str() } else { "" };
                lines.push(Line::from(vec![
                    Span::styled(format!("{key:column$}"), Style::new().fg(ORANGE).bold()),
                    Span::raw(row),
                ]));
            }
        }
    }
    lines
}

/// Draws the `?` overlay, when open, centered over `body`: a bordered box
/// titled `Keys`, as wide as its widest line where there is room, and as tall
/// as its content or the body. The cells under the box are cleared first, so
/// nothing shows through. The click regions underneath are dropped and the
/// box is the only one left, so a click can't reach what the help covers,
/// and `hits.max_scroll` becomes the help's. The scroll is clamped to the
/// content and written back.
fn render_help(frame: &mut Frame, app: &mut App, body: Rect, hits: &mut Hits) {
    let Some(help) = app.help else {
        return;
    };
    hits.regions.clear();
    let width = body
        .width
        .min(u16::try_from(help_width() + 4).unwrap_or(u16::MAX));
    let lines = help_lines(app, usize::from(width.saturating_sub(4)));
    let total = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let height = body.height.min(total.saturating_add(2));
    let area = Rect {
        x: body.x + (body.width - width) / 2,
        y: body.y + (body.height - height) / 2,
        width,
        height,
    };
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(ORANGE)
        .title(Span::styled(" Keys ", Style::new().fg(Color::White).bold()))
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);
    let max_scroll = total.saturating_sub(inner.height);
    let scroll = help.scroll.min(max_scroll);
    app.help = Some(Help { scroll });
    hits.max_scroll = max_scroll;
    if max_scroll > 0 && inner.height > 0 {
        let last = scroll + inner.height;
        block = block.title_bottom(
            Line::styled(
                format!(" {}-{last} of {total} ", scroll + 1),
                Style::new().fg(DIM),
            )
            .right_aligned(),
        );
    }
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)).block(block), area);
    if !area.is_empty() {
        hits.regions.push((area, Target::Help));
    }
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
        (
            item("⊘ Hidden".to_owned(), Feed::Hidden),
            Some(Feed::Hidden),
        ),
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
        Feed::Home | Feed::Popular | Feed::All | Feed::Saved | Feed::Hidden => wrap(
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
        Feed::Home | Feed::Popular | Feed::All | Feed::Saved | Feed::Hidden => " Create Post ",
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
        Feed::Home | Feed::Popular | Feed::All | Feed::Saved | Feed::Hidden => app.feed_name(),
    };

    // Hidden posts are left out here too, so the next one takes the place.
    let mut trending: Vec<&Post> = app.posts.iter().filter(|p| !p.hidden).collect();
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
    let mut tabs_end = x;
    for (&sort, spans) in Sort::ALL.iter().zip(tab_spans.chunks(3)) {
        let [number, label, gap] = [0, 1, 2].map(|i| spans[i].width() as u16);
        let rect = Rect::new(x + 1, tab_row.y, number - 1 + label, 1).intersection(tab_row);
        if !rect.is_empty() {
            hits.regions.push((rect, Target::Sort(sort)));
        }
        tabs_end = x + number + label;
        x += number + label + gap;
    }
    frame.render_widget(Paragraph::new(Line::from(tab_spans)).block(tab_block), tabs);

    // The view switch, right-aligned with a blank column before the border.
    // Drawn, and clickable, only with at least one blank column between it
    // and the last tab.
    let switch = density_label(app.density);
    let width = switch.width() as u16;
    let at = tab_row.right().saturating_sub(width + 1);
    if tab_row.width > width && at > tabs_end {
        let rect = Rect::new(at, tab_row.y, width, 1);
        frame.render_widget(switch, rect);
        hits.regions.push((rect, Target::Density));
    }

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
        } else if app.feed == Feed::Hidden && !app.posts.iter().any(|p| p.hidden) {
            ("No hidden posts", "Press h on any post to hide it")
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

    let heights: Vec<u16> = match app.density {
        Density::Card => {
            let content_width = usize::from(cards.width.saturating_sub(VOTE_WIDTH + 4));
            app.view
                .iter()
                .map(|&i| card_height(&app.posts[i], content_width))
                .collect()
        }
        Density::Compact => vec![1; app.view.len()],
    };

    // Scroll just far enough that the selected card (or row) is fully visible.
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
        match app.density {
            Density::Card => render_card(frame, app, vi, &app.posts[pi], rect, hits),
            Density::Compact => render_compact_row(frame, app, vi, &app.posts[pi], rect, hits),
        }
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

/// The tab box's view switch: a glyph and the current view's name.
fn density_label(density: Density) -> Line<'static> {
    let (glyph, name) = match density {
        Density::Card => ("▦", "Card"),
        Density::Compact => ("≡", "Compact"),
    };
    Line::from(vec![
        Span::styled(format!("{glyph} "), Style::new().fg(ORANGE)),
        Span::styled(name, Style::new().fg(Color::White).bold()),
    ])
}

/// The first `width` cells of `spans`. Every glyph here is one cell wide.
fn clip(spans: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    let mut left = width;
    let mut out = Vec::new();
    for mut span in spans {
        if left == 0 {
            break;
        }
        let len = span.content.chars().count();
        if len > left {
            span.content = span.content.chars().take(left).collect::<String>().into();
        }
        left -= len.min(left);
        out.push(span);
    }
    out
}

/// The cells of a compact row a click acts on, as columns within the row.
struct RowCells {
    up: u16,
    down: u16,
    star: u16,
}

/// A compact row, at most `width` cells: ` ▲ <score> ▼ ☆ <title>`, then the
/// dim `w/<group> · <age> · ▤ <comments>` right-aligned with a blank column
/// after it. The meta shortens so the title keeps `MIN_TITLE` columns of
/// room when it can: first the group goes, then the comment count, then the
/// whole meta. Only then is the title's room below `MIN_TITLE`. Also returns
/// the columns of the arrows and the star, which may be past `width`.
fn compact_row(app: &App, post: &Post, selected: bool, width: usize) -> (Line<'static>, RowCells) {
    let arrow = |glyph: &'static str, vote: Vote, color: Color| {
        let color = if post.vote == vote { color } else { DIM };
        Span::styled(glyph, Style::new().fg(color).bold())
    };
    let star = if post.saved {
        Span::styled("★", Style::new().fg(ORANGE).bold())
    } else {
        Span::styled("☆", Style::new().fg(DIM).bold())
    };
    let mut spans = vec![
        Span::raw(" "),
        arrow("▲", Vote::Up, ORANGE),
        Span::styled(
            format!(" {:>SCORE_WIDTH$} ", compact(i64::from(post.score))),
            Style::new().fg(vote_color(post.vote)).bold(),
        ),
        arrow("▼", Vote::Down, PERIWINKLE),
        Span::raw(" "),
    ];
    let column = |spans: &[Span]| spans.iter().map(Span::width).sum::<usize>() as u16;
    let up = 1;
    let down = column(&spans) - 2;
    let star_at = column(&spans);
    spans.extend([star, Span::raw(" ")]);
    let title_at = usize::from(column(&spans));

    let group = format!("w/{}", app.groups[post.group].name);
    let age = age(post.hours_ago);
    let comments = format!("▤ {}", compact(i64::from(post.comment_count)));
    // The title's room with each meta: 2 blank columns before the meta and 1
    // after it, or just the 1 when there is none.
    let room = |meta: &str| {
        let meta = meta.chars().count();
        width.saturating_sub(title_at + 1 + if meta > 0 { meta + 2 } else { 0 })
    };
    let meta = [
        format!("{group} · {age} · {comments}"),
        format!("{age} · {comments}"),
        age.clone(),
    ]
    .into_iter()
    .find(|meta| room(meta) >= MIN_TITLE)
    .unwrap_or_default();
    let room = room(&meta);

    // Whitespace runs become one space, as on cards, which is what lets
    // `highlight_rows` walk the title alongside the row.
    let title = post.title.split_whitespace().collect::<Vec<_>>().join(" ");
    let len = title.chars().count();
    let (shown, from_text) = if len <= room {
        (title, len)
    } else if room == 0 {
        (String::new(), 0)
    } else {
        (truncate(&title, room), room.saturating_sub(1))
    };
    let shown_len = shown.chars().count();
    let title_style = Style::new()
        .fg(if selected { ORANGE } else { Color::White })
        .bold();
    let line = highlight_rows(
        post.title,
        &app.search.highlights(post.title),
        vec![(shown, from_text)],
        title_style,
    )
    .remove(0);
    spans.extend(line.spans.into_iter().map(|mut span| {
        span.style = line.style.patch(span.style);
        span
    }));
    if !meta.is_empty() {
        spans.push(Span::raw(" ".repeat(room - shown_len + 2)));
        spans.push(Span::styled(meta, Style::new().fg(DIM)));
    }

    let cells = RowCells {
        up,
        down,
        star: star_at,
    };
    (Line::from(clip(spans, width)), cells)
}

/// Draws the compact row for `view[index]`, recording its arrows and star.
fn render_compact_row(
    frame: &mut Frame,
    app: &App,
    index: usize,
    post: &Post,
    area: Rect,
    hits: &mut Hits,
) {
    let selected = index == app.selected;
    let (line, cells) = compact_row(app, post, selected, usize::from(area.width));
    let bg = if selected { HIGHLIGHT_BG } else { Color::Reset };
    frame.render_widget(Paragraph::new(line).bg(bg), area);
    for (column, target) in [
        (cells.up, Target::Vote(index, Vote::Up)),
        (cells.down, Target::Vote(index, Vote::Down)),
        (cells.star, Target::Save(index)),
    ] {
        if column < area.width {
            hits.regions
                .push((Rect::new(area.x + column, area.y, 1, 1), target));
        }
    }
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

    use ratatui::layout::Position;

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

    /// The footer while the help is open, trimmed.
    const HELP_FOOTER: &str = " j/k scroll  g/G top/bottom  ?/esc close";

    #[test]
    fn every_hint_set_fits_80_columns_on_every_feed() {
        let mut app = App::new();
        app.posts[0].saved = true;
        // Hidden needs a post to open too, from a group with another one left.
        let hidden = (1..app.posts.len())
            .find(|&i| {
                let group = app.posts[i].group;
                (0..app.posts.len()).any(|j| j != i && app.posts[j].group == group)
            })
            .expect("some seed group has two posts");
        app.posts[hidden].hidden = true;
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

        press(&mut app, KeyCode::Char('1'));
        let feeds = 5 + app.groups.len();
        let mut visited = Vec::new();
        for _ in 0..feeds {
            for density in [Density::Card, Density::Compact] {
                if app.density != density {
                    press(&mut app, KeyCode::Char('v'));
                    let toast = match density {
                        Density::Card => " ✔ Card view",
                        Density::Compact => " ✔ Compact view",
                    };
                    assert_eq!(footer(&mut app), toast, "{:?}", app.feed);
                    // Esc with no kept search does nothing but clear the toast.
                    press(&mut app, KeyCode::Esc);
                }
                let feed = app.feed;
                visited.push(feed);
                let on_group = matches!(feed, Feed::Group(_));
                let feed = (feed, density);

                let plain = footer(&mut app);
                assert!(plain.ends_with("q quit"), "{feed:?}: {plain:?}");
                assert!(plain.contains(" h hide "), "{feed:?}: {plain:?}");
                assert!(plain.contains(" ? help "), "{feed:?}: {plain:?}");
                // Counting the blank after the last hint, as PLAN.md does.
                assert!(plain.chars().count() < 80, "{feed:?}: {plain:?}");
                assert_eq!(plain.contains("J join"), on_group, "{feed:?}: {plain:?}");
                let expected = if on_group {
                    " a/z vote  1-4 sort  [/] feed  / search  s save  h hide  J join  ? help  q quit"
                } else {
                    " a/z vote  1-4 sort  [/] feed  / search  s save  h hide  ? help  q quit"
                };
                assert_eq!(plain, expected, "{feed:?}");

                press(&mut app, KeyCode::Char('?'));
                let help = footer(&mut app);
                assert_eq!(help, HELP_FOOTER, "{feed:?}");
                press(&mut app, KeyCode::Char('?'));
                assert_eq!(footer(&mut app), plain, "{feed:?}: help closed");

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
                    post, " j/k scroll  n/p comments  a/z vote  s save  c share  ? help  esc back",
                    "{feed:?}"
                );
                press(&mut app, KeyCode::Char('?'));
                assert_eq!(footer(&mut app), HELP_FOOTER, "{feed:?}: post view");
                press(&mut app, KeyCode::Esc);
                assert_eq!(footer(&mut app), post, "{feed:?}: help closed");

                press(&mut app, KeyCode::Char('n'));
                assert!(app.thread.focus.is_some(), "{feed:?}");
                let focused = footer(&mut app);
                assert_eq!(
                    focused, " n/p comments  a/z vote comment  enter collapse  ? help  esc unfocus",
                    "{feed:?}"
                );
                assert!(focused.chars().count() < 80, "{feed:?}: {focused:?}");
                press(&mut app, KeyCode::Char('?'));
                assert_eq!(footer(&mut app), HELP_FOOTER, "{feed:?}: focused");
                press(&mut app, KeyCode::Char('q'));
                assert_eq!(footer(&mut app), focused, "{feed:?}: help closed");
                press(&mut app, KeyCode::Char('c'));
                assert_eq!(footer(&mut app), " ✔ Link copied to clipboard!", "{feed:?}");
                press(&mut app, KeyCode::Esc);
                assert_eq!(footer(&mut app), post, "{feed:?}: unfocused again");
                press(&mut app, KeyCode::Esc);
                assert_eq!(app.mode, Mode::Feed, "{feed:?}");
                assert_eq!(app.density, density, "{feed:?}: back in the same view");
            }

            press(&mut app, KeyCode::Char(']'));
        }
        assert_eq!(app.feed, Feed::Home, "visited every feed");
        assert!(visited.contains(&Feed::Hidden), "{visited:?}");
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
        type_str(&mut app, "]]]]]");
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
                (app.density, app.sort, app.feed, app.mode, &app.query),
                (&app.search, app.help),
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

    /// Presses `]` until `feed` is on screen.
    fn go_to(app: &mut App, feed: Feed) {
        for _ in 0..6 + app.groups.len() {
            if app.feed == feed {
                return;
            }
            press(app, KeyCode::Char(']'));
        }
        assert_eq!(app.feed, feed);
    }

    /// The selection and offset are in bounds, and the selected card is on
    /// screen after a render.
    fn assert_scroll_in_bounds(app: &mut App, why: &str) {
        let check = |app: &App, when: &str| {
            assert!(app.offset <= app.selected, "{why} {when}");
            assert!(
                app.view.is_empty() || app.selected < app.view.len(),
                "{why} {when}"
            );
            assert!(app.offset <= app.view.len(), "{why} {when}");
        };
        check(app, "before the render");
        let s = screen(app, 110, 24);
        check(app, "after the render");
        assert!(
            s.contains(&format!(" {} posts ", app.view.len())),
            "{why}:\n{s}"
        );
        if !app.view.is_empty() {
            let title = title_of(app, app.selected);
            assert!(s.contains(&title), "{why}: {title:?} not drawn:\n{s}");
        }
    }

    #[test]
    fn hiding_near_the_bottom_keeps_the_offset_in_bounds() {
        let mut app = App::new();
        go_to(&mut app, Feed::All);
        let total = app.view.len();
        press(&mut app, KeyCode::Char('G'));
        assert_scroll_in_bounds(&mut app, "after G");
        assert!(app.offset > 0, "scrolled");

        // One above the last card, then the last card, render after each key.
        press(&mut app, KeyCode::Char('k'));
        assert_scroll_in_bounds(&mut app, "after k");
        let next = app.view[app.selected + 1];
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(app.view[app.selected], next, "the next card moved up");
        assert_scroll_in_bounds(&mut app, "after hiding the second to last");
        assert_eq!(app.view.len(), total - 1);

        let mut left = total - 1;
        while left > 0 {
            assert_eq!(app.selected, left - 1, "the last card is selected");
            press(&mut app, KeyCode::Char('h'));
            left -= 1;
            assert_eq!(app.view.len(), left);
            assert_scroll_in_bounds(&mut app, &format!("{left} left"));
        }
        let s = screen(&mut app, 110, 24);
        assert!(
            s.contains(" 0 posts ") && s.contains("Nothing here yet"),
            "{s}"
        );
    }

    #[test]
    fn hiding_every_card_of_a_group_feed_counts_down_to_nothing_here_yet() {
        let mut app = App::new();
        go_to(&mut app, Feed::Group(0));
        let mut left = app.view.len();
        assert!(left >= 2, "{left}");
        while left > 0 {
            let s = screen(&mut app, 110, 24);
            assert!(s.contains(&format!(" {left} posts ")), "{s}");
            press(&mut app, KeyCode::Char('h'));
            left -= 1;
            assert_eq!(app.view.len(), left);
        }
        let s = screen(&mut app, 110, 24);
        assert!(s.contains(" 0 posts "), "{s}");
        assert!(s.contains("Nothing here yet"), "{s}");
        assert!(s.contains("Post hidden"), "{s}");
    }

    /// The right sidebar's text, at 140 columns.
    fn sidebar(app: &mut App) -> String {
        rows(&buffer(app, 140, 40))
            .iter()
            .map(|row| row.chars().skip(140 - 32).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn hidden_posts_leave_trending_today() {
        let mut app = App::new();
        go_to(&mut app, Feed::All);
        let mut by_comments: Vec<usize> = (0..app.posts.len()).collect();
        by_comments.sort_by_key(|&i| std::cmp::Reverse(app.posts[i].comment_count));
        let titles: Vec<String> = app.posts.iter().map(|p| truncate(p.title, 28)).collect();
        let shown = |i: usize| titles[i].clone();
        let [top, .., fifth] = by_comments[..5] else {
            panic!("seed has 5 posts");
        };
        let (top_title, fifth_title) = (shown(top), shown(fifth));

        let s = sidebar(&mut app);
        assert!(s.contains("Trending today"), "{s}");
        assert!(s.contains(&top_title), "{s}");
        assert!(!s.contains(&fifth_title), "{s}");

        app.selected = app.view.iter().position(|&i| i == top).expect("in All");
        press(&mut app, KeyCode::Char('h'));
        assert!(app.posts[top].hidden);
        let s = sidebar(&mut app);
        assert!(!s.contains(&top_title), "the hidden post is gone:\n{s}");
        assert!(
            s.contains(&fifth_title),
            "the next one takes its place:\n{s}"
        );
        for &i in &by_comments[1..4] {
            assert!(s.contains(&shown(i)), "{s}");
        }
    }

    #[test]
    fn saved_says_no_saved_posts_only_when_nothing_is_saved() {
        let mut app = App::new();
        go_to(&mut app, Feed::Saved);
        let s = screen(&mut app, 110, 24);
        assert!(s.contains("No saved posts yet"), "{s}");

        // Save the first Home post, then hide it: Saved is empty, not unused.
        go_to(&mut app, Feed::Home);
        type_str(&mut app, "sh");
        go_to(&mut app, Feed::Saved);
        assert!(app.view.is_empty());
        let s = screen(&mut app, 110, 24);
        assert!(s.contains("Nothing here yet"), "{s}");
        assert!(!s.contains("No saved posts yet"), "{s}");
        assert!(s.contains(" 0 posts "), "{s}");
    }

    #[test]
    fn hidden_empty_messages() {
        let mut app = App::new();
        go_to(&mut app, Feed::Hidden);
        let s = screen(&mut app, 110, 24);
        assert!(s.contains("No hidden posts"), "{s}");
        assert!(s.contains("Press h on any post to hide it"), "{s}");
        assert!(!s.contains("Nothing here yet"), "{s}");

        // Hide a post a day old, then let Rising or a search filter it out.
        go_to(&mut app, Feed::All);
        app.selected = app
            .view
            .iter()
            .position(|&i| app.posts[i].hours_ago >= 24)
            .expect("seed data has posts a day old");
        press(&mut app, KeyCode::Char('h'));
        go_to(&mut app, Feed::Hidden);
        assert_eq!(app.view.len(), 1);
        press(&mut app, KeyCode::Char('4'));
        assert!(app.view.is_empty());
        let s = screen(&mut app, 110, 24);
        assert!(s.contains("Nothing here yet"), "Rising:\n{s}");
        assert!(!s.contains("No hidden posts"), "{s}");

        press(&mut app, KeyCode::Char('1'));
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "zzzz");
        let s = screen(&mut app, 110, 24);
        assert!(s.contains("couldn't find any results"), "search:\n{s}");
        assert!(!s.contains("No hidden posts"), "{s}");
        press(&mut app, KeyCode::Enter);
        let s = screen(&mut app, 110, 24);
        assert!(s.contains("couldn't find any results"), "kept:\n{s}");
    }

    #[test]
    fn nav_shows_hidden_after_saved_and_a_click_opens_it() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "w/");
        press(&mut app, KeyCode::Enter);
        let buf = buffer(&mut app, 110, 24);
        let (sx, sy) = find(&buf, "☆ Saved");
        let (hx, hy) = find(&buf, "⊘ Hidden");
        assert_eq!((hx, hy), (sx, sy + 1), "right below Saved");

        click(&mut app, (hx + 12, hy));
        assert_eq!(app.feed, Feed::Hidden);
        assert_eq!(app.query, "w/", "a kept search stays");
        let buf = buffer(&mut app, 110, 24);
        let s = rows(&buf).join("\n");
        for needle in [
            "╭ Hidden ",
            "Your personal workit",
            "Create Post",
            "No hidden posts",
        ] {
            assert!(s.contains(needle), "missing {needle:?} in:\n{s}");
        }
        assert_eq!(buf[(hx, hy)].fg, ORANGE, "the active entry");

        click(&mut app, (sx, sy));
        assert_eq!(app.feed, Feed::Saved);
    }

    #[test]
    fn a_click_after_hiding_hits_the_card_drawn_there() {
        let mut app = App::new();
        buffer(&mut app, 110, 24);
        let gone = app.view[0];
        let next = app.view[1];
        let score = app.posts[next].score;
        press(&mut app, KeyCode::Char('h'));
        let buf = buffer(&mut app, 110, 24);
        let (x, y) = find(&buf, &title_of(&app, 0));
        assert_eq!(app.view[0], next);

        // The ▲ of the first card drawn: two rows above the score, left of it.
        let (_, arrow_y) = find(&buf, "▲");
        let arrow_x = (0..110)
            .find(|&x| buf[(x, arrow_y)].symbol() == "▲")
            .expect("drawn");
        click(&mut app, (arrow_x, arrow_y));
        assert_eq!(app.posts[next].score, score + 1, "voted on the new card");
        assert_eq!(app.posts[gone].vote, Vote::None, "not the hidden one");

        buffer(&mut app, 110, 24);
        click(&mut app, (x, y));
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
        assert_eq!(app.view[app.selected], next, "opened the card drawn there");
    }

    /// The seed app in the Compact view, toast cleared.
    fn compact_app() -> App {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('v'));
        assert_eq!(app.density, Density::Compact);
        app.status = None;
        app
    }

    /// The text of a compact row's line for `post` at `width`.
    fn row_text(app: &App, post: &Post, width: usize) -> String {
        compact_row(app, post, false, width)
            .0
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect()
    }

    /// The x of the first `glyph` at or right of `from` on row `y`.
    fn glyph_x(buf: &Buffer, glyph: &str, from: u16, y: u16) -> u16 {
        (from..buf.area.width)
            .find(|&x| buf[(x, y)].symbol() == glyph)
            .unwrap_or_else(|| panic!("no {glyph} on row {y}:\n{}", rows(buf).join("\n")))
    }

    #[test]
    fn compact_row_exact_text_with_save_and_vote_colors() {
        let mut app = compact_app();
        // The first Home post upvoted and saved, the second downvoted.
        type_str(&mut app, "sajzk");
        app.status = None;
        let buf = buffer(&mut app, 140, 30);
        let main: Vec<String> = rows(&buf)
            .iter()
            .map(|r| r.chars().skip(22).take(86).collect())
            .collect();
        assert_eq!(
            main[6],
            " ▲  1.2k ▼ ★ We deleted 40k lines of dead code and t…  w/Engineering · 3h ago · ▤ 194 "
        );
        assert_eq!(
            main[7],
            " ▲   855 ▼ ☆ PSA: the espresso machine on floor 3 is …  w/OfficeLife · 1h ago · ▤ 100 "
        );

        // Upvoted and saved: ▲, score and ★ orange. Downvoted: ▼ and score
        // periwinkle. The other arrow dim, the meta dim.
        let (up, score, down, star) = (23, 26, 31, 33);
        assert_eq!(buf[(up, 6)].fg, ORANGE);
        assert_eq!(buf[(score, 6)].fg, ORANGE);
        assert_eq!(buf[(down, 6)].fg, DIM);
        assert_eq!((buf[(star, 6)].symbol(), buf[(star, 6)].fg), ("★", ORANGE));
        assert_eq!(buf[(up, 7)].fg, DIM);
        assert_eq!(buf[(score, 7)].fg, PERIWINKLE);
        assert_eq!(buf[(down, 7)].fg, PERIWINKLE);
        assert_eq!((buf[(star, 7)].symbol(), buf[(star, 7)].fg), ("☆", DIM));
        assert_eq!(buf[(23, 8)].fg, DIM, "no vote: dim arrows");
        assert_eq!(
            buf[(26, 8)].fg,
            Color::White,
            "and a white score, as on cards"
        );
        let meta = find(&buf, "w/OfficeLife · 1h ago");
        assert_eq!(
            (buf[meta].fg, buf[meta].modifier.contains(Modifier::BOLD)),
            (DIM, false)
        );

        // The selected row: orange bold title on HIGHLIGHT_BG, the whole row
        // wide. The others: white bold title, no background.
        assert_eq!(app.selected, 0);
        for x in 22..108 {
            assert_eq!(buf[(x, 6)].bg, HIGHLIGHT_BG, "selected row, x {x}");
            assert_eq!(buf[(x, 7)].bg, Color::Reset, "unselected row, x {x}");
        }
        let title = (35, 6);
        assert_eq!(buf[title].fg, ORANGE);
        assert!(buf[title].modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(35, 7)].fg, Color::White);
        assert!(buf[(35, 7)].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn compact_meta_shortens_in_order_and_rows_never_overflow() {
        let app = App::new();
        let post = &app.posts[app.view[0]];
        assert_eq!(post.title.chars().count(), 62);
        let stage = |w: usize| {
            let text = row_text(&app, post, w);
            if text.contains("w/Engineering · 3h ago · ▤ 194") {
                0
            } else if text.contains("3h ago · ▤ 194") {
                1
            } else if text.contains("3h ago") {
                2
            } else {
                3
            }
        };
        // 13 columns before the title, 2 between it and the meta, 1 after.
        // The full meta is 30 wide, without the group 14, the age alone 6.
        assert_eq!((stage(66), stage(65)), (0, 1), "the group goes first");
        assert_eq!((stage(50), stage(49)), (1, 2), "then the comment count");
        assert_eq!((stage(42), stage(41)), (2, 3), "then the whole meta");
        let mut last = 0;
        for w in (1..=120).rev() {
            assert!(stage(w) >= last, "the meta came back at {w}");
            last = stage(w);
        }
        assert_eq!(
            row_text(&app, post, 65),
            " ▲  1.2k ▼ ☆ We deleted 40k lines of dead code …  3h ago · ▤ 194"
        );
        // The title keeps 20 columns as long as it can, then is cut.
        let title_room = |w: usize| row_text(&app, post, w).chars().count() - 13;
        assert_eq!(title_room(41), 27, "all but the blank column at the end");
        assert_eq!(title_room(34), 20);
        assert_eq!(row_text(&app, post, 33), " ▲  1.2k ▼ ☆ We deleted 40k lin…");
        assert_eq!(row_text(&app, post, 15), " ▲  1.2k ▼ ☆ …");
        assert_eq!(row_text(&app, post, 14), " ▲  1.2k ▼ ☆ ", "no room, no …");
        assert_eq!(row_text(&app, post, 5), " ▲  1");
        assert_eq!(row_text(&app, post, 1), " ");

        for w in 1..=120 {
            for (i, post) in app.posts.iter().enumerate() {
                for selected in [false, true] {
                    let width = compact_row(&app, post, selected, w).0.width();
                    assert!(width <= w, "post {i} is {width} wide at {w}");
                }
            }
        }
    }

    #[test]
    fn compact_rows_draw_inside_their_area_at_every_width() {
        let mut app = compact_app();
        type_str(&mut app, "sa");
        for width in 1..=120u16 {
            let buf = buffer(&mut app, width, 24);
            let hits = app.hits.clone().expect("rendered");
            let rows = hits
                .regions
                .iter()
                .filter(|(_, t)| matches!(t, Target::Card(_)))
                .collect::<Vec<_>>();
            for (rect, target) in &hits.regions {
                if let Target::Vote(i, _) | Target::Save(i) = target {
                    let (row, _) = rows
                        .iter()
                        .find(|(_, t)| *t == Target::Card(*i))
                        .expect("a row for every arrow and star");
                    assert_eq!(rect.intersection(*row), *rect, "{width}: {target:?}");
                    assert_eq!((rect.width, rect.height), (1, 1));
                    let glyph = buf[(rect.x, rect.y)].symbol();
                    assert!(["▲", "▼", "☆", "★"].contains(&glyph), "{width}: {glyph:?}");
                }
            }
            // Past the right edge of the main column is the sidebar or nothing.
            for (row, _) in rows {
                assert!(row.right() <= width, "{width}: {row:?}");
                assert_eq!(row.height, 1);
            }
        }
    }

    #[test]
    fn compact_arrows_line_up_for_any_score_width() {
        let mut app = compact_app();
        let post = |score, title| Post {
            group: 0,
            author: "dana_k",
            title,
            score,
            hours_ago: 1,
            ..Default::default()
        };
        app.posts = vec![
            post(7, "seven"),
            post(845, "eight four five"),
            post(1_243, "one point two k"),
            post(-12, "negative"),
        ];
        press(&mut app, KeyCode::Char('3'));
        app.status = None;
        let buf = buffer(&mut app, 110, 24);
        let ys: Vec<u16> = ["seven", "eight four five", "one point two k", "negative"]
            .iter()
            .map(|t| find(&buf, t).1)
            .collect();
        let columns = |glyph| {
            ys.iter()
                .map(|&y| glyph_x(&buf, glyph, 22, y))
                .collect::<Vec<_>>()
        };
        let up = columns("▲");
        let down = columns("▼");
        let star = columns("☆");
        assert!(up.iter().all(|&x| x == up[0]), "{up:?}");
        assert!(down.iter().all(|&x| x == down[0]), "{down:?}");
        assert!(star.iter().all(|&x| x == star[0]), "{star:?}");
        let titles: Vec<u16> = ["seven", "eight four five", "one point two k", "negative"]
            .iter()
            .map(|t| find(&buf, t).0)
            .collect();
        assert!(titles.iter().all(|&x| x == titles[0]), "{titles:?}");
        let clusters = ["▲     7 ▼ ☆", "▲   845 ▼ ☆", "▲  1.2k ▼ ☆", "▲   -12 ▼ ☆"];
        for (&y, cluster) in ys.iter().zip(clusters) {
            let row: String = rows(&buf)[usize::from(y)]
                .chars()
                .skip(23)
                .take(11)
                .collect();
            assert_eq!(row, cluster);
        }

        // Every seed score, voted up or down, fits the column.
        for p in data::posts() {
            for score in [p.score - 1, p.score, p.score + 1] {
                assert!(compact(i64::from(score)).chars().count() <= SCORE_WIDTH);
            }
        }
    }

    #[test]
    fn compact_title_highlights_including_a_match_cut_by_the_ellipsis() {
        // 60 columns, no nav: "1h ago · ▤ 0" leaves the title 32 columns,
        // "aaaa bbbb cccc dddd eeee ffff g…", so gggg is cut after its g.
        let mut app = one_post("aaaa bbbb cccc dddd eeee ffff gggg hhhh", "");
        press(&mut app, KeyCode::Char('v'));
        let plain = buffer(&mut app, 60, 20);
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "BBBB gggg");
        assert_eq!(app.view, [0]);
        let buf = buffer(&mut app, 60, 20);
        let (x, y) = find(&buf, "aaaa bbbb cccc dddd eeee ffff g…");
        assert_eq!(x, 13);
        let expected = format!("{}^^^^{}^.", ".".repeat(5), ".".repeat(21));
        assert_eq!(hits(&buf, x, y, 32), expected, "the … is never marked");
        assert_eq!(
            (buf[(x + 31, y)].fg, buf[(x + 31, y)].bg),
            (ORANGE, HIGHLIGHT_BG),
            "the … keeps the selected title style"
        );
        for dx in 0..32 {
            assert!(buf[(x + dx, y)].modifier.contains(Modifier::BOLD), "{dx}");
        }
        let (mx, my) = find(&buf, "1h ago · ▤ 0");
        assert_eq!(hits(&buf, mx, my, 12), "............", "the meta isn't");
        assert_eq!(rows(&buf)[usize::from(y)], rows(&plain)[usize::from(y)]);

        // Every occurrence, in the original case, overlapping terms merged
        // into one run, in a kept search too.
        let mut app = one_post("Old HUGE post about huge things", "");
        type_str(&mut app, "v/huge \"ge p\"");
        let buf = buffer(&mut app, 60, 20);
        let (x, y) = find(&buf, "Old HUGE post about huge things");
        assert_eq!(hits(&buf, x, y, 31), "....^^^^^^..........^^^^.......");
        press(&mut app, KeyCode::Enter);
        let kept = buffer(&mut app, 60, 20);
        assert_eq!(hits(&kept, x, y, 31), "....^^^^^^..........^^^^.......");
    }

    #[test]
    fn compact_rows_with_filters_alone_render_like_no_query() {
        let mut app = compact_app();
        type_str(&mut app, "]]]]]");
        assert_eq!(app.feed, Feed::Group(0));
        app.status = None;
        let plain = buffer(&mut app, 110, 24);
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "W/eng u/ \" \"");
        press(&mut app, KeyCode::Enter);
        let filtered = buffer(&mut app, 110, 24);
        for y in 3..23 {
            for x in 0..110 {
                assert_eq!(plain[(x, y)], filtered[(x, y)], "cell ({x}, {y})");
            }
        }
    }

    /// The posts drawn in the main column: one `▲` each, in cards or rows.
    fn posts_drawn(buf: &Buffer) -> usize {
        find_all(buf, "▲").len()
    }

    /// The selected post's row in the Compact view is on screen and drawn in
    /// the selected style.
    fn assert_selected_row_on_screen(app: &mut App, why: &str) {
        let buf = buffer(app, 110, 24);
        assert!(app.offset <= app.selected, "{why}");
        assert!(app.selected - app.offset < 17, "{why}: 17 rows fit");
        let (x, y) = find(&buf, &title_of(app, app.selected));
        assert_eq!(y, 6 + (app.selected - app.offset) as u16, "{why}");
        assert_eq!(
            (buf[(x, y)].fg, buf[(x, y)].bg),
            (ORANGE, HIGHLIGHT_BG),
            "{why}"
        );
    }

    #[test]
    fn compact_view_shows_many_more_posts_and_keeps_the_selection_on_screen() {
        let mut app = App::new();
        let cards = posts_drawn(&buffer(&mut app, 110, 24));
        press(&mut app, KeyCode::Char('v'));
        let buf = buffer(&mut app, 110, 24);
        let compact = posts_drawn(&buf);
        assert_eq!((cards, compact), (2, 17), "{}", rows(&buf).join("\n"));
        assert_eq!(app.view.len(), 19);

        press(&mut app, KeyCode::Char('G'));
        assert_selected_row_on_screen(&mut app, "after G");
        assert_eq!(app.offset, 2, "advanced just far enough");
        press(&mut app, KeyCode::Char('k'));
        assert_selected_row_on_screen(&mut app, "after k");
        assert_eq!(app.offset, 2, "no scroll needed");
        press(&mut app, KeyCode::Char('g'));
        assert_selected_row_on_screen(&mut app, "after g");
        assert_eq!(app.offset, 0);
        for (n, selected) in [5, 10, 15, 18].into_iter().enumerate() {
            press(&mut app, KeyCode::PageDown);
            assert_eq!(app.selected, selected, "PgDn moves 5 posts");
            assert_selected_row_on_screen(&mut app, &format!("PgDn {n}"));
        }
        press(&mut app, KeyCode::PageUp);
        assert_eq!(app.selected, 13);
        assert_selected_row_on_screen(&mut app, "PgUp");
        for _ in 0..13 {
            press(&mut app, KeyCode::Char('k'));
            assert_selected_row_on_screen(&mut app, "walking up with k");
        }
        assert_eq!(app.offset, 0);
    }

    #[test]
    fn switching_to_cards_with_the_last_post_selected_shows_that_card_whole() {
        let mut app = compact_app();
        press(&mut app, KeyCode::Char('G'));
        buffer(&mut app, 110, 24);
        let compact_offset = app.offset;
        let last = app.selected;
        press(&mut app, KeyCode::Char('v'));
        assert_eq!(app.status, Some("Card view"));
        assert_eq!(
            (app.selected, app.offset),
            (last, compact_offset),
            "left to the renderer"
        );

        // The first frame in the Card view scrolls the card fully on screen.
        let buf = buffer(&mut app, 110, 24);
        let s = rows(&buf).join("\n");
        // 56 columns for the feed, 17 rows below the tab box.
        let heights: u16 = app.view[app.offset..=last]
            .iter()
            .map(|&i| card_height(&app.posts[i], 56 - usize::from(VOTE_WIDTH) - 4))
            .sum();
        assert!(heights <= 17, "{heights}:\n{s}");
        assert!(app.offset > compact_offset, "scrolled further:\n{s}");
        let post = &app.posts[app.view[last]];
        assert!(s.contains(&title_of(&app, last)), "{s}");
        let comments = format!("▤ {} Comments", compact(i64::from(post.comment_count)));
        let (_, y) = find(&buf, &comments);
        assert_eq!(buf[(23, y + 1)].symbol(), "─", "its bottom border:\n{s}");
        assert!(y < 22, "inside the feed, which ends at row 22:\n{s}");
    }

    #[test]
    fn clicking_compact_rows_opens_votes_and_saves() {
        let mut app = compact_app();
        type_str(&mut app, "]]");
        assert_eq!(app.feed, Feed::All);
        press(&mut app, KeyCode::Char('G'));
        buffer(&mut app, 110, 24);
        let offset = app.offset;
        assert!(offset > 0, "scrolled: {offset}");
        press(&mut app, KeyCode::Char('k'));
        let buf = buffer(&mut app, 110, 24);
        assert_eq!(app.offset, offset);

        // The first row drawn is view[offset], not view[0].
        let (x, y) = find(&buf, &title_of(&app, offset));
        click(&mut app, (x + 3, y));
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
        assert_eq!(app.selected, offset);
        assert_eq!(app.thread, Default::default());
        press(&mut app, KeyCode::Esc);
        assert_eq!((app.density, app.selected), (Density::Compact, offset));

        // The meta and the blank cells count as the row.
        let buf = buffer(&mut app, 110, 24);
        let (_, y) = find(&buf, &title_of(&app, offset + 2));
        click(&mut app, (75, y));
        assert_eq!(
            (app.mode, app.selected),
            (Mode::Post { scroll: 0 }, offset + 2)
        );
        press(&mut app, KeyCode::Esc);

        // ▲ and ▼ select the row and vote, without opening or re-sorting.
        let view = app.view.clone();
        let buf = buffer(&mut app, 110, 24);
        let (_, y) = find(&buf, &title_of(&app, offset + 1));
        let i = app.view[offset + 1];
        let base = app.posts[i].score;
        let up = (glyph_x(&buf, "▲", 22, y), y);
        click(&mut app, up);
        assert_eq!(app.selected, offset + 1);
        assert_eq!(
            (app.posts[i].vote, app.posts[i].score),
            (Vote::Up, base + 1)
        );
        assert_eq!(app.mode, Mode::Feed, "not opened");
        let buf = buffer(&mut app, 110, 24);
        assert_eq!(buf[up].fg, ORANGE);
        let down = (glyph_x(&buf, "▼", 22, y), y);
        assert_eq!(down.0, up.0 + 8);
        click(&mut app, down);
        assert_eq!(
            (app.posts[i].vote, app.posts[i].score),
            (Vote::Down, base - 1)
        );
        assert_eq!(app.view, view, "voting never re-sorts");

        // ☆ selects the row and saves it, ★ unsaves.
        let buf = buffer(&mut app, 110, 24);
        let (_, y) = find(&buf, &title_of(&app, offset));
        let j = app.view[offset];
        let star = (glyph_x(&buf, "☆", 22, y), y);
        click(&mut app, star);
        assert!(app.posts[j].saved);
        assert_eq!((app.selected, app.mode), (offset, Mode::Feed));
        assert_eq!(app.status, Some("Post saved"));
        let buf = buffer(&mut app, 110, 24);
        assert_eq!(buf[star].symbol(), "★");
        click(&mut app, star);
        assert!(!app.posts[j].saved);
        assert_eq!(app.status, Some("Post unsaved"));

        // The blank cell between ▲ and the score opens the row.
        buffer(&mut app, 110, 24);
        click(&mut app, (up.0 + 1, up.1));
        assert_eq!(
            (app.mode, app.selected),
            (Mode::Post { scroll: 0 }, offset + 1)
        );
    }

    #[test]
    fn after_a_toggle_a_click_hits_what_the_new_view_draws() {
        let mut app = App::new();
        let cards = buffer(&mut app, 110, 24);
        // The second card's ▲ is on a row that a compact row covers.
        let up = find_all(&cards, "▲")[1];
        press(&mut app, KeyCode::Char('v'));
        let buf = buffer(&mut app, 110, 24);
        assert_ne!(buf[up].symbol(), "▲");
        let row = (usize::from(up.1) - 6) + app.offset;
        click(&mut app, up);
        assert_eq!((app.mode, app.selected), (Mode::Post { scroll: 0 }, row));
        assert!(
            app.posts.iter().all(|p| p.vote == Vote::None),
            "no card vote"
        );

        // And back: a compact row's ☆ cell is inside a card again.
        press(&mut app, KeyCode::Esc);
        let buf = buffer(&mut app, 110, 24);
        let star = (glyph_x(&buf, "☆", 22, 9), 9);
        press(&mut app, KeyCode::Char('v'));
        buffer(&mut app, 110, 24);
        click(&mut app, star);
        assert!(app.posts.iter().all(|p| !p.saved), "no compact save");
        assert_eq!(app.mode, Mode::Post { scroll: 0 }, "opened the card there");
    }

    #[test]
    fn wheel_moves_the_selection_in_compact_view() {
        let mut app = compact_app();
        buffer(&mut app, 110, 24);
        mouse(&mut app, MouseEventKind::ScrollDown, (40, 10));
        mouse(&mut app, MouseEventKind::ScrollDown, (40, 10));
        mouse(&mut app, MouseEventKind::ScrollUp, (5, 10));
        assert_eq!((app.selected, app.mode), (1, Mode::Feed));
        for _ in 0..30 {
            mouse(&mut app, MouseEventKind::ScrollDown, (40, 10));
        }
        assert_eq!(app.selected, app.view.len() - 1);
        assert_selected_row_on_screen(&mut app, "after the wheel");
    }

    fn density_regions(app: &App) -> usize {
        let hits = app.hits.as_ref().expect("rendered");
        hits.regions
            .iter()
            .filter(|(_, t)| *t == Target::Density)
            .count()
    }

    #[test]
    fn view_switch_label_is_drawn_and_clickable_where_it_fits() {
        let mut app = App::new();
        let buf = buffer(&mut app, 110, 24);
        let (x, y) = find(&buf, "▦ Card");
        assert_eq!((x, y), (70, 4), "right-aligned, a blank before the border");
        assert_eq!(buf[(76, 4)].symbol(), " ");
        assert_eq!(buf[(77, 4)].symbol(), "│");
        assert_eq!(density_regions(&app), 1);
        click(&mut app, (x + 3, y));
        assert_eq!(
            (app.density, app.status),
            (Density::Compact, Some("Compact view"))
        );
        let buf = buffer(&mut app, 110, 24);
        let (x, y) = find(&buf, "≡ Compact");
        assert_eq!((x, y), (67, 4));
        assert!(find_all(&buf, "▦").is_empty());
        click(&mut app, (x, y));
        assert_eq!(
            (app.density, app.status),
            (Density::Card, Some("Card view"))
        );
        assert_eq!(app.mode, Mode::Feed);

        // At 46 columns (no nav) "≡ Compact" just fits, one blank after
        // "Rising"; at 45 it would touch it, so it isn't drawn or clickable.
        press(&mut app, KeyCode::Char('v'));
        let buf = buffer(&mut app, 46, 24);
        assert_eq!(
            rows(&buf)[4],
            "│ 1 Hot   2 New   3 Top   4 Rising ≡ Compact │"
        );
        assert_eq!(density_regions(&app), 1);
        let buf = buffer(&mut app, 45, 24);
        assert_eq!(
            rows(&buf)[4],
            "│ 1 Hot   2 New   3 Top   4 Rising          │"
        );
        assert_eq!(density_regions(&app), 0);
        click(&mut app, (36, 4));
        click(&mut app, (43, 4));
        assert_eq!((app.density, app.mode), (Density::Compact, Mode::Feed));
        // "▦ Card" is shorter: it still fits at 43 but not at 42.
        press(&mut app, KeyCode::Char('v'));
        let buf = buffer(&mut app, 43, 24);
        assert_eq!(rows(&buf)[4], "│ 1 Hot   2 New   3 Top   4 Rising ▦ Card │");
        let buf = buffer(&mut app, 42, 24);
        assert!(find_all(&buf, "▦").is_empty(), "{}", rows(&buf)[4]);
        assert_eq!(density_regions(&app), 0);
        click(&mut app, (36, 4));
        assert_eq!(app.density, Density::Card);
        for width in 1..=120 {
            buffer(&mut app, width, 24);
            assert!(density_regions(&app) <= 1);
        }
    }

    #[test]
    fn view_switch_does_nothing_in_the_post_view() {
        let mut app = App::new();
        let buf = buffer(&mut app, 110, 24);
        let (x, y) = find(&buf, "▦ Card");
        press(&mut app, KeyCode::Enter);
        let post = buffer(&mut app, 110, 24);
        assert!(find_all(&post, "▦").is_empty());
        assert_eq!(density_regions(&app), 0);
        click(&mut app, (x, y));
        click(&mut app, (x + 4, y));
        assert_eq!((app.density, app.status), (Density::Card, None));
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
    }

    #[test]
    fn view_switch_glyphs_are_one_cell_and_used_nowhere_else() {
        for density in [Density::Card, Density::Compact] {
            let label = density_label(density);
            assert_eq!(
                label.width(),
                label.to_string().chars().count(),
                "{density:?}"
            );
            let glyph = label.to_string().chars().next().expect("a glyph");
            assert_eq!(Span::raw(glyph.to_string()).width(), 1, "{glyph}");
        }
        // Every screen: Card and Compact feeds, the post view, a sidebar.
        let mut app = App::new();
        app.posts[0].saved = true;
        let card = rows(&buffer(&mut app, 140, 40)).join("\n");
        assert_eq!(
            (card.matches('▦').count(), card.matches('≡').count()),
            (1, 0)
        );
        press(&mut app, KeyCode::Char('v'));
        let compact = rows(&buffer(&mut app, 140, 40)).join("\n");
        assert_eq!(
            (compact.matches('▦').count(), compact.matches('≡').count()),
            (0, 1)
        );
        press(&mut app, KeyCode::Enter);
        let post = rows(&buffer(&mut app, 140, 60)).join("\n");
        assert!(!post.contains('▦') && !post.contains('≡'));
    }

    #[test]
    fn hiding_in_compact_view_removes_exactly_that_row() {
        let mut app = compact_app();
        go_to(&mut app, Feed::All);
        press(&mut app, KeyCode::Char('j'));
        let before: Vec<String> = (0..app.view.len()).map(|i| title_of(&app, i)).collect();
        let buf = buffer(&mut app, 110, 24);
        let (_, y) = find(&buf, &before[1]);
        assert_eq!(y, 7);
        press(&mut app, KeyCode::Char('h'));
        let buf = buffer(&mut app, 110, 24);
        let s = rows(&buf).join("\n");
        assert!(!s.contains(&before[1]), "{s}");
        assert!(s.contains(&format!(" {} posts ", before.len() - 1)), "{s}");
        assert_eq!(app.selected, 1, "the next row takes its place");
        assert_eq!(find(&buf, &before[2]).1, 7, "and its row");
        assert_eq!(find(&buf, &before[0]).1, 6);
        let after: Vec<String> = (0..app.view.len()).map(|i| title_of(&app, i)).collect();
        let mut expected = before.clone();
        expected.remove(1);
        assert_eq!(after, expected, "the others keep their order");
        assert_eq!(app.density, Density::Compact);
    }

    /// The help's box, as recorded by the last frame.
    fn help_box(app: &App) -> Option<Rect> {
        let hits = app.hits.as_ref()?;
        hits.regions
            .iter()
            .find(|(_, target)| *target == Target::Help)
            .map(|(rect, _)| *rect)
    }

    fn help_scroll(app: &App) -> u16 {
        app.help.expect("the help is open").scroll
    }

    fn help_text(app: &App, width: usize) -> Vec<String> {
        help_lines(app, width)
            .iter()
            .map(|line| line.to_string())
            .collect()
    }

    /// The text inside the box's borders and padding, one string per row.
    fn help_rows(buf: &Buffer, area: Rect) -> Vec<String> {
        (area.y + 1..area.bottom().saturating_sub(1))
            .map(|y| {
                (area.x + 2..area.right().saturating_sub(2))
                    .map(|x| buf[(x, y)].symbol())
                    .collect()
            })
            .collect()
    }

    /// The section headings of the help, in the order shown.
    fn headings(app: &App) -> Vec<String> {
        let titles: Vec<&str> = HELP.iter().map(|(_, title, _)| *title).collect();
        help_text(app, usize::MAX)
            .into_iter()
            .filter(|line| titles.contains(&line.as_str()))
            .collect()
    }

    #[test]
    fn help_box_is_centered_over_the_body_and_hides_what_is_under_it() {
        let mut app = App::new();
        let closed = buffer(&mut app, 80, 24);
        press(&mut app, KeyCode::Char('?'));
        let buf = buffer(&mut app, 80, 24);
        let area = help_box(&app).expect("the box is drawn");
        let width = help_width() as u16 + 4;
        assert!(width <= 80, "{width}");
        assert_eq!(area, Rect::new((80 - width) / 2, 3, width, 20));
        assert_eq!(find(&buf, " Keys "), (area.x + 1, 3), "the title");
        assert_eq!(buf[(area.x, 3)].symbol(), "╭");
        assert_eq!(buf[(area.right() - 1, 22)].symbol(), "╯");

        let (before, after) = (rows(&closed), rows(&buf));
        assert_eq!(after[..3], before[..3], "the header stays");
        assert_eq!(after[23].trim_end(), HELP_FOOTER, "the footer stays");
        for y in 3..23 {
            for x in 0..80 {
                if !area.contains(Position::new(x, y)) {
                    assert_eq!(buf[(x, y)], closed[(x, y)], "underneath at {x},{y}");
                }
            }
        }
        // Inside, exactly the help's lines, padded with blanks: nothing from
        // the cards shows through, and no line is cut off.
        let inner = usize::from(width - 4);
        let text = help_text(&app, inner);
        assert_eq!(text, help_text(&app, usize::MAX), "nothing wraps at 80");
        let shown = help_rows(&buf, area);
        assert_eq!(shown.len(), 18);
        for (row, line) in shown.iter().zip(&text) {
            assert_eq!(*row, format!("{line:inner$}"));
        }
        for y in 4..22 {
            assert_eq!(buf[(area.x + 1, y)].symbol(), " ", "left padding");
            assert_eq!(buf[(area.right() - 2, y)].symbol(), " ", "right padding");
        }
        assert_eq!(text[0], "Feed", "the feed's section first");
        assert!(
            after[22].contains(&format!(" 1-18 of {} ", text.len())),
            "{}",
            after[22]
        );
    }

    #[test]
    fn the_section_for_the_current_mode_comes_first() {
        let others = ["Search mode", "Mouse", "This help", "Anywhere"];
        let mut app = thread_post();
        press(&mut app, KeyCode::Char('?'));
        assert_eq!(headings(&app)[..2], ["Feed", "Post view"]);
        assert_eq!(headings(&app)[2..], others);
        press(&mut app, KeyCode::Esc);

        type_str(&mut app, "/thread");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('?'));
        assert_eq!(headings(&app)[0], "Feed", "with a kept search");
        press(&mut app, KeyCode::Esc);

        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Char('?'));
        assert_eq!(headings(&app)[..2], ["Post view", "Feed"]);
        assert_eq!(headings(&app)[2..], others);
        let buf = buffer(&mut app, 80, 24);
        let area = help_box(&app).expect("drawn");
        assert_eq!(help_rows(&buf, area)[0].trim_end(), "Post view");
        assert!(
            help_rows(&buf, area)[1].starts_with("  j/k, ↓/↑       scroll 1 line"),
            "{:?}",
            help_rows(&buf, area)[1]
        );
    }

    #[test]
    fn no_help_line_is_cut_off_at_80_columns_or_wider() {
        let descriptions: Vec<&str> = HELP
            .iter()
            .flat_map(|(_, _, rows)| rows.iter().map(|(_, what)| *what))
            .collect();
        for (width, height) in [(80, 24), (100, 30), (110, 24), (200, 70)] {
            for post in [false, true] {
                let mut app = App::new();
                if post {
                    press(&mut app, KeyCode::Enter);
                }
                press(&mut app, KeyCode::Char('?'));
                let mut seen: Vec<String> = Vec::new();
                loop {
                    let buf = buffer(&mut app, width, height);
                    let area = help_box(&app).expect("drawn");
                    assert_eq!(area.width, help_width() as u16 + 4, "{width}x{height}");
                    seen.extend(
                        help_rows(&buf, area)
                            .into_iter()
                            .map(|row| row.trim_end().into()),
                    );
                    if help_scroll(&app) == app.hits.as_ref().unwrap().max_scroll {
                        break;
                    }
                    press(&mut app, KeyCode::PageDown);
                }
                for line in help_text(&app, usize::MAX) {
                    assert!(seen.contains(&line), "{width}x{height} {post}: {line:?}");
                }
                for what in &descriptions {
                    assert!(
                        seen.iter().any(|row| row.ends_with(what)),
                        "{width}x{height}: {what:?}"
                    );
                }
            }
        }
        // Tall enough for all of it: no scrolling and no position.
        let mut app = App::new();
        press(&mut app, KeyCode::Char('?'));
        let buf = buffer(&mut app, 200, 70);
        assert_eq!(app.hits.as_ref().unwrap().max_scroll, 0);
        let area = help_box(&app).unwrap();
        assert_eq!(area.height, help_text(&app, usize::MAX).len() as u16 + 2);
        let bottom = &rows(&buf)[usize::from(area.bottom() - 1)];
        assert!(!bottom.contains(" of "), "{bottom}");
    }

    #[test]
    fn help_keys_scroll_it_clamped_to_the_content() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('?'));
        let draw = |app: &mut App| {
            buffer(app, 80, 24);
            help_scroll(app)
        };
        assert_eq!(draw(&mut app), 0);
        let total = help_text(&app, usize::MAX).len() as u16;
        let max = app.hits.as_ref().unwrap().max_scroll;
        assert_eq!(max, total - 18, "18 rows inside the box");

        let mut step = |code: KeyCode| {
            press(&mut app, code);
            draw(&mut app)
        };
        assert_eq!(step(KeyCode::Char('k')), 0, "clamped at the top");
        assert_eq!(step(KeyCode::Up), 0);
        assert_eq!(step(KeyCode::PageUp), 0);
        assert_eq!(step(KeyCode::Char('j')), 1);
        assert_eq!(step(KeyCode::Down), 2);
        assert_eq!(step(KeyCode::Up), 1);
        assert_eq!(step(KeyCode::PageDown), 11);
        assert_eq!(step(KeyCode::Char(' ')), 21);
        assert_eq!(step(KeyCode::PageUp), 11);
        assert_eq!(step(KeyCode::Char('g')), 0);
        assert_eq!(step(KeyCode::End), max);
        assert_eq!(step(KeyCode::Home), 0);
        assert_eq!(step(KeyCode::Char('G')), max);
        assert_eq!(step(KeyCode::Char('j')), max, "clamped at the bottom");
        assert_eq!(step(KeyCode::PageDown), max);
        assert_eq!(step(KeyCode::Char('k')), max - 1, "G then k moves at once");

        press(&mut app, KeyCode::Char('G'));
        assert_eq!(help_scroll(&app), u16::MAX, "before the renderer clamps it");
        let buf = buffer(&mut app, 80, 24);
        assert_eq!(help_scroll(&app), max);
        let area = help_box(&app).unwrap();
        let shown = help_rows(&buf, area);
        assert!(shown[17].starts_with("  Ctrl-C  quit"), "{shown:?}");
        assert!(rows(&buf)[22].contains(&format!(" {}-{total} of {total} ", max + 1)));
        assert_eq!(app.mode, Mode::Feed);
        assert_eq!((app.selected, app.offset), (0, 0), "the feed never moved");
    }

    #[test]
    fn wheel_scrolls_the_help_three_lines_and_is_clamped() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('?'));
        buffer(&mut app, 80, 24);
        let max = app.hits.as_ref().unwrap().max_scroll;
        let area = help_box(&app).unwrap();
        let inside = (area.x + area.width / 2, area.y + area.height / 2);
        let mut wheel = |kind: MouseEventKind, at: (u16, u16)| {
            mouse(&mut app, kind, at);
            buffer(&mut app, 80, 24);
            help_scroll(&app)
        };
        assert_eq!(wheel(MouseEventKind::ScrollDown, inside), 3);
        assert_eq!(wheel(MouseEventKind::ScrollDown, (0, 0)), 6, "anywhere");
        assert_eq!(wheel(MouseEventKind::ScrollUp, (5, 10)), 3);
        assert_eq!(wheel(MouseEventKind::ScrollUp, inside), 0);
        assert_eq!(
            wheel(MouseEventKind::ScrollUp, inside),
            0,
            "clamped at the top"
        );
        for _ in 0..max {
            wheel(MouseEventKind::ScrollDown, inside);
        }
        assert_eq!(help_scroll(&app), max, "clamped at the bottom");
        assert_eq!(app.hits.as_ref().unwrap().max_scroll, max);
        mouse(&mut app, MouseEventKind::ScrollDown, inside);
        assert_eq!(help_scroll(&app), max, "clamped straight away");
        assert_eq!(
            (app.selected, app.mode),
            (0, Mode::Feed),
            "the feed never moved"
        );

        // In the post view too, and the post doesn't scroll.
        let mut app = App::new();
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('?'));
        buffer(&mut app, 80, 24);
        mouse(&mut app, MouseEventKind::ScrollDown, (40, 12));
        assert_eq!(help_scroll(&app), 3);
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
    }

    #[test]
    fn a_click_outside_the_help_closes_it_and_does_nothing_else() {
        let feed = || {
            let mut app = App::new();
            app.posts[0].saved = true;
            app
        };
        let post = || {
            let mut app = thread_post();
            press(&mut app, KeyCode::Enter);
            app
        };
        // Where the help-free frame drew a card, its arrow and Save, a tab,
        // the view switch, a nav entry, the search box and a comment header.
        let cases: [(&dyn Fn() -> App, &str); 9] = [
            (&feed, "We deleted 40k"),
            (&feed, "▲"),
            (&feed, "▼"),
            (&feed, "★ Saved"),
            (&feed, "New"),
            (&feed, "▦ Card"),
            (&feed, "◎ All"),
            (&feed, "Search Home"),
            (&post, "u/dan"),
        ];
        for (setup, needle) in cases {
            let mut app = setup();
            let closed = buffer(&mut app, 200, 40);
            let at = find(&closed, needle);
            let before = state(&app);
            press(&mut app, KeyCode::Char('?'));
            buffer(&mut app, 200, 40);
            let area = help_box(&app).unwrap();
            assert!(!area.contains(Position::new(at.0, at.1)), "{needle}");
            click(&mut app, at);
            assert_eq!(app.help, None, "{needle}");
            assert_eq!(app.status, None, "{needle}");
            buffer(&mut app, 200, 40);
            assert_eq!(state(&app), before, "{needle}: nothing else changed");
        }

        // Even when no frame has been drawn since `?`, so the last one still
        // has the card's regions.
        let mut app = feed();
        let closed = buffer(&mut app, 200, 40);
        let before = state(&app);
        press(&mut app, KeyCode::Char('?'));
        click(&mut app, find(&closed, "We deleted 40k"));
        assert_eq!(app.help, None);
        buffer(&mut app, 200, 40);
        assert_eq!(state(&app), before);
    }

    /// The target the last frame recorded at `at`, as `on_mouse` sees it.
    fn region_at(app: &App, at: (u16, u16)) -> Option<Target> {
        let regions = &app.hits.as_ref()?.regions;
        regions
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(Position::new(at.0, at.1)))
            .map(|(_, target)| target.clone())
    }

    #[test]
    fn before_the_next_frame_a_click_on_a_stale_region_only_closes_the_help() {
        // `?` changes no region: until the next frame, `hits` still holds the
        // feed's or the post view's, and no `Help` box. Every click is then
        // outside the help, so it closes it and does nothing more, even on a
        // cell the stale regions say is an arrow, a tab, a nav entry or a
        // comment header (and where the box will be drawn).
        let feed = || App::new();
        let thread = || {
            let mut app = thread_post();
            press(&mut app, KeyCode::Enter);
            app
        };
        let focused = || {
            let mut app = thread();
            press(&mut app, KeyCode::Char('n'));
            app
        };
        let cases: [(&dyn Fn() -> App, &str, Target); 7] = [
            (&feed, "▲", Target::Vote(0, Vote::Up)),
            (&feed, "▼", Target::Vote(0, Vote::Down)),
            (&feed, "New", Target::Sort(Sort::New)),
            (&feed, "◎ All", Target::Feed(Feed::All)),
            (&feed, "We deleted 40k", Target::Card(0)),
            (&thread, "u/bob", Target::Comment(vec![0, 0])),
            // The focused header: a click would collapse it.
            (&focused, "u/ann", Target::Comment(vec![0])),
        ];
        for (setup, needle, expected) in cases {
            let mut app = setup();
            let buf = buffer(&mut app, 80, 24);
            let at = find(&buf, needle);
            let target = region_at(&app, at)
                .unwrap_or_else(|| panic!("{needle} at {at:?}:\n{}", rows(&buf).join("\n")));
            assert_eq!(target, expected, "{needle}");
            let before = state(&app);
            press(&mut app, KeyCode::Char('?'));
            assert_eq!(region_at(&app, at), Some(target), "{needle}: still there");
            click(&mut app, at);
            assert_eq!(app.help, None, "{needle}: closed");
            assert_eq!(state(&app), before, "{needle}: nothing else");
            buffer(&mut app, 80, 24);
            assert_eq!(state(&app), before, "{needle}: after a frame");
        }
        // The wheel doesn't fall through to the feed or the post either.
        for setup in [&feed as &dyn Fn() -> App, &thread] {
            let mut app = setup();
            buffer(&mut app, 80, 24);
            press(&mut app, KeyCode::Char('?'));
            let (selected, mode) = (app.selected, app.mode);
            mouse(&mut app, MouseEventKind::ScrollDown, (40, 12));
            assert_eq!((app.selected, app.mode), (selected, mode));
            assert!(app.help.is_some());
        }
    }

    #[test]
    fn resizing_with_the_help_open_reclamps_its_scroll() {
        let mut app = App::new();
        press(&mut app, KeyCode::Char('?'));
        buffer(&mut app, 80, 24);
        press(&mut app, KeyCode::Char('G'));
        for (width, height) in [(80, 24), (80, 50), (30, 8), (80, 70), (80, 24)] {
            press(&mut app, KeyCode::Char('G'));
            let buf = buffer(&mut app, width, height);
            let area = help_box(&app).expect("drawn");
            let inner = usize::from(area.height - 2);
            let text = help_text(&app, usize::from(area.width - 4));
            let max = text.len().saturating_sub(inner);
            let why = format!("{width}x{height}");
            assert_eq!(usize::from(help_scroll(&app)), max, "{why}");
            assert_eq!(
                usize::from(app.hits.as_ref().unwrap().max_scroll),
                max,
                "{why}"
            );
            let shown = help_rows(&buf, area);
            assert_eq!(shown.len(), inner, "{why}");
            // The last line is on the box's last row: no blank after it.
            assert_eq!(
                shown.last().unwrap().trim_end(),
                text.last().unwrap().trim_end(),
                "{why}"
            );
            if text.len() + 2 <= usize::from(height - 4) {
                assert_eq!((max, inner), (0, text.len()), "{why}: fits whole");
            }
            // `k` moves on the very next frame.
            press(&mut app, KeyCode::Char('k'));
            buffer(&mut app, width, height);
            assert_eq!(
                usize::from(help_scroll(&app)),
                max.saturating_sub(1),
                "{why}"
            );
        }

        // Growing without pressing anything clamps a scroll left at the
        // old bottom, and shrinking keeps it.
        let mut app = App::new();
        press(&mut app, KeyCode::Char('?'));
        press(&mut app, KeyCode::Char('G'));
        buffer(&mut app, 80, 24);
        let small = help_scroll(&app);
        buffer(&mut app, 80, 50);
        let big = help_scroll(&app);
        assert!(big < small, "{big} < {small}");
        let inner = usize::from(help_box(&app).unwrap().width - 4);
        assert_eq!(usize::from(big), help_text(&app, inner).len() - 44);
        buffer(&mut app, 80, 70);
        assert_eq!(help_scroll(&app), 0);
        buffer(&mut app, 30, 8);
        assert_eq!(
            help_scroll(&app),
            0,
            "a smaller box doesn't scroll by itself"
        );
        for (width, height) in [(1, 1), (2, 5), (30, 4), (80, 24)] {
            press(&mut app, KeyCode::Char('G'));
            buffer(&mut app, width, height);
            assert!(help_scroll(&app) <= app.hits.as_ref().unwrap().max_scroll);
        }
    }

    #[test]
    fn a_click_inside_the_help_does_nothing() {
        for post in [false, true] {
            let mut app = thread_post();
            if post {
                press(&mut app, KeyCode::Enter);
            }
            press(&mut app, KeyCode::Char('?'));
            buffer(&mut app, 80, 24);
            let area = help_box(&app).unwrap();
            let before = state(&app);
            // Corners, the title, the padding, text, and over the card or
            // comment headers drawn underneath.
            let (right, bottom) = (area.right() - 1, area.bottom() - 1);
            for at in [
                (area.x, area.y),
                (right, bottom),
                (area.x + 2, area.y),
                (area.x + 1, area.y + 1),
                (area.x + 4, area.y + 2),
                (area.x + area.width / 2, area.y + area.height / 2),
                (right, area.y + 5),
            ] {
                click(&mut app, at);
                assert_eq!(state(&app), before, "{post} {at:?}");
                buffer(&mut app, 80, 24);
                assert_eq!(state(&app), before, "{post} {at:?}");
            }
        }
    }

    #[test]
    fn help_opens_anywhere_but_search_and_closing_restores_everything() {
        let render = |app: &mut App| {
            buffer(app, 80, 24);
        };
        let scrolled_cards = || {
            let mut app = App::new();
            press(&mut app, KeyCode::Char('3'));
            press(&mut app, KeyCode::Char('G'));
            press(&mut app, KeyCode::Char('k'));
            app
        };
        let scrolled_rows = || {
            let mut app = compact_app();
            for _ in 0..20 {
                press(&mut app, KeyCode::Char('j'));
            }
            app
        };
        let group_with_search = || {
            let mut app = App::new();
            go_to(&mut app, Feed::Group(0));
            type_str(&mut app, "/e");
            press(&mut app, KeyCode::Enter);
            press(&mut app, KeyCode::Char('j'));
            app
        };
        let empty = || {
            let mut app = App::new();
            go_to(&mut app, Feed::Hidden);
            app
        };
        let thread = || {
            let mut app = App::new();
            press(&mut app, KeyCode::Enter);
            press(&mut app, KeyCode::Char('n'));
            press(&mut app, KeyCode::Enter);
            press(&mut app, KeyCode::Char('n'));
            press(&mut app, KeyCode::Char('j'));
            press(&mut app, KeyCode::Char('j'));
            app
        };
        let cases: [(&dyn Fn() -> App, &str); 5] = [
            (&scrolled_cards, "cards"),
            (&scrolled_rows, "compact rows"),
            (&group_with_search, "group feed, kept search"),
            (&empty, "empty feed"),
            (&thread, "post view"),
        ];
        for (setup, name) in cases {
            for close in [KeyCode::Char('?'), KeyCode::Esc, KeyCode::Char('q')] {
                let mut app = setup();
                app.status = None;
                render(&mut app);
                match name {
                    "cards" | "compact rows" => assert!(app.offset > 0, "{name}"),
                    "group feed, kept search" => {
                        assert!(!app.query.is_empty() && app.selected == 1, "{name}")
                    }
                    "empty feed" => assert!(app.view.is_empty(), "{name}"),
                    _ => {
                        assert!(matches!(app.mode, Mode::Post { scroll } if scroll > 0));
                        assert!(app.thread.focus.is_some() && !app.thread.collapsed.is_empty());
                    }
                }
                let before = state(&app);
                press(&mut app, KeyCode::Char('?'));
                assert_eq!(app.help, Some(Help { scroll: 0 }), "{name}");
                render(&mut app);
                assert!(help_box(&app).is_some(), "{name}");
                press(&mut app, KeyCode::Char('j'));
                render(&mut app);
                assert_eq!(help_scroll(&app), 1, "{name}");
                press(&mut app, close);
                assert_eq!(app.help, None, "{name} {close:?}");
                assert!(!app.should_quit, "{name} {close:?}");
                render(&mut app);
                assert_eq!(state(&app), before, "{name} {close:?}");
            }
        }
    }

    #[test]
    fn the_help_swallows_every_other_key() {
        let keys = [
            "a", "z", "s", "h", "J", "v", "c", "n", "p", "1", "2", "3", "4", "[", "]", "/", "x",
            "Q", "K",
        ]
        .map(|key| KeyCode::Char(key.chars().next().unwrap()))
        .into_iter()
        .chain([
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::BackTab,
            KeyCode::Backspace,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Delete,
            KeyCode::Insert,
            KeyCode::F(1),
        ]);
        let group = || {
            let mut app = App::new();
            go_to(&mut app, Feed::Group(1));
            app
        };
        let compact_search = || {
            let mut app = compact_app();
            type_str(&mut app, "/the");
            press(&mut app, KeyCode::Enter);
            app
        };
        let post = || {
            let mut app = thread_post();
            press(&mut app, KeyCode::Enter);
            press(&mut app, KeyCode::Char('n'));
            app
        };
        let cases: [&dyn Fn() -> App; 3] = [&group, &compact_search, &post];
        for setup in cases {
            let mut app = setup();
            assert!(!app.view.is_empty());
            press(&mut app, KeyCode::Char('?'));
            buffer(&mut app, 80, 24);
            let before = state(&app);
            for key in keys.clone() {
                press(&mut app, key);
                buffer(&mut app, 80, 24);
                assert_eq!(state(&app), before, "{key:?}");
            }
        }
    }

    #[test]
    fn mouse_events_the_app_ignores_change_nothing_with_the_help_open() {
        let rejected = [
            MouseEventKind::Moved,
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::Down(MouseButton::Right),
            MouseEventKind::Up(MouseButton::Right),
            MouseEventKind::Down(MouseButton::Middle),
            MouseEventKind::ScrollLeft,
            MouseEventKind::ScrollRight,
        ];
        for post in [false, true] {
            let mut app = thread_post();
            if post {
                press(&mut app, KeyCode::Enter);
            }
            press(&mut app, KeyCode::Char('c'));
            press(&mut app, KeyCode::Char('?'));
            app.status = Some("Link copied to clipboard!");
            buffer(&mut app, 110, 24);
            let area = help_box(&app).unwrap();
            let center = (area.x + area.width / 2, area.y + area.height / 2);
            for at in [center, (5, 5), (60, 1), (0, 23), (area.x, area.y)] {
                for kind in rejected {
                    let event = MouseEvent {
                        kind,
                        column: at.0,
                        row: at.1,
                        modifiers: KeyModifiers::NONE,
                    };
                    assert!(!app.handles_mouse(&event), "{kind:?}");
                    let before = state(&app);
                    app.on_mouse(event);
                    assert_eq!(state(&app), before, "{post}: {kind:?} at {at:?}");
                }
                for kind in [
                    MouseEventKind::Down(MouseButton::Left),
                    MouseEventKind::ScrollDown,
                    MouseEventKind::ScrollUp,
                ] {
                    let event = MouseEvent {
                        kind,
                        column: at.0,
                        row: at.1,
                        modifiers: KeyModifiers::NONE,
                    };
                    assert!(app.handles_mouse(&event), "{kind:?}");
                }
            }
        }
    }

    #[test]
    fn help_fits_the_body_at_every_size_and_never_panics() {
        let footer = |width: u16| -> String {
            let line = format!("{HELP_FOOTER} ");
            let cut: String = line.chars().take(usize::from(width)).collect();
            format!("{cut:w$}", w = usize::from(width))
        };
        let sizes = (1..=32).flat_map(|w| (1..=12).map(move |h| (w, h))).chain([
            (80, 4),
            (80, 5),
            (200, 4),
            (30, 24),
            (79, 24),
            (63, 22),
        ]);
        for post in [false, true] {
            let mut app = App::new();
            if post {
                press(&mut app, KeyCode::Enter);
            }
            for (width, height) in sizes.clone() {
                let why = format!("{width}x{height} post={post}");
                let [_, body, footer_area] = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(1),
                ])
                .areas(Rect::new(0, 0, width, height));
                let closed = buffer(&mut app, width, height);
                press(&mut app, KeyCode::Char('?'));
                let buf = buffer(&mut app, width, height);
                match help_box(&app) {
                    Some(area) => {
                        assert_eq!(area.intersection(body), area, "{why}: {area:?}")
                    }
                    None => assert!(body.is_empty(), "{why}: no box"),
                }
                for y in 0..height {
                    let row: String = (0..width).map(|x| buf[(x, y)].symbol()).collect();
                    // With under 2 body rows the feed's view switch lands on
                    // the footer row, help or not, so only the help's absence
                    // from it is checked there (it isn't drawn at all).
                    if footer_area.height > 0 && y == footer_area.y {
                        if body.height < 2 {
                            continue;
                        }
                        assert_eq!(row, footer(width), "{why}: the footer");
                    } else if !(body.y..body.bottom()).contains(&y) {
                        let was: String = (0..width).map(|x| closed[(x, y)].symbol()).collect();
                        assert_eq!(row, was, "{why}: row {y} isn't the body's");
                    }
                }
                press(&mut app, KeyCode::Char('G'));
                buffer(&mut app, width, height);
                assert!(help_scroll(&app) <= app.hits.as_ref().unwrap().max_scroll);
                press(&mut app, KeyCode::Char('?'));
                assert_eq!(app.help, None);
            }
        }
    }

    #[test]
    fn every_footer_key_is_in_the_help_table() {
        // A key matches exactly, or ignoring case when it is a name rather
        // than a letter (`enter` and `Enter`, but `J` isn't `j`).
        let listed = |section: Section, key: &str| {
            HELP.iter()
                .filter(|(s, ..)| *s == section)
                .flat_map(|(_, _, rows)| rows.iter())
                .flat_map(|(keys, _)| keys.iter())
                .any(|k| *k == key || (key.chars().count() > 1 && k.eq_ignore_ascii_case(key)))
        };
        let sets: [(&[Hint], Section); 7] = [
            (FEED_HINTS, Section::Feed),
            (GROUP_HINTS, Section::Feed),
            (KEPT_SEARCH_HINTS, Section::Feed),
            (POST_HINTS, Section::Post),
            (FOCUSED_HINTS, Section::Post),
            (SEARCH_HINTS, Section::Search),
            (HELP_HINTS, Section::Help),
        ];
        for (set, section) in sets {
            for (key, _) in set {
                assert!(listed(section, key), "{section:?} is missing {key:?}");
            }
        }
        // What the feed's footer leaves out is in the help.
        for key in ["enter", "j/k", "g/G", "PgDn/PgUp", "Tab", "c", "v", "esc"] {
            assert!(listed(Section::Feed, key), "{key}");
        }
        assert!(listed(Section::Anywhere, "Ctrl-C"));
        assert!(listed(Section::Search, "Backspace"));
        assert!(listed(Section::Post, "Backspace") && listed(Section::Post, "Space"));
        // Each section once.
        for section in [
            Section::Feed,
            Section::Post,
            Section::Search,
            Section::Mouse,
            Section::Help,
            Section::Anywhere,
        ] {
            assert_eq!(HELP.iter().filter(|(s, ..)| *s == section).count(), 1);
        }
        // And the footer only ever shows one of those sets.
        let mut app = App::new();
        let check = |app: &App| {
            let shown = hints(app);
            assert!(sets.iter().any(|(set, _)| *set == shown), "{shown:?}");
        };
        for keys in [
            "", "?", "?q/e", "\n", "\n", "\nn", "\nn?", "q", "\n\x1b", "[[[[[[[",
        ] {
            for ch in keys.chars() {
                press(
                    &mut app,
                    match ch {
                        '\n' => KeyCode::Enter,
                        '\x1b' => KeyCode::Esc,
                        ch => KeyCode::Char(ch),
                    },
                );
                check(&app);
            }
        }
    }

    #[test]
    fn help_glyphs_are_one_cell_wide() {
        let mut texts: Vec<&str> = vec![" Keys "];
        for (_, title, rows) in HELP {
            texts.push(title);
            for (keys, what) in rows {
                texts.extend(keys.iter());
                texts.push(what);
            }
        }
        for set in [
            FEED_HINTS,
            GROUP_HINTS,
            KEPT_SEARCH_HINTS,
            POST_HINTS,
            FOCUSED_HINTS,
            SEARCH_HINTS,
            HELP_HINTS,
        ] {
            for (key, what) in set {
                texts.extend([*key, *what]);
            }
        }
        let all = texts.concat();
        for glyph in ['↓', '↑', '▲', '▼', '☆'] {
            assert!(all.contains(glyph), "{glyph}");
        }
        for text in texts {
            assert_eq!(Line::raw(text).width(), text.chars().count(), "{text:?}");
            for ch in text.chars() {
                assert_eq!(Span::raw(ch.to_string()).width(), 1, "{ch:?} in {text:?}");
            }
        }
        // The view switch's glyphs stay the switch's alone.
        assert!(!all.contains('▦') && !all.contains('≡'));
    }
}
