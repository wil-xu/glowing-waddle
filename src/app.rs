use std::cmp::Reverse;
use std::collections::HashSet;
use std::ops::Range;

use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::crossterm::event::MouseButton;
use ratatui::crossterm::event::MouseEvent;
use ratatui::crossterm::event::MouseEventKind;
use ratatui::layout::Position;
use ratatui::layout::Rect;

use crate::data;
use crate::data::Comment;
use crate::data::Group;
use crate::data::Post;
use crate::data::Vote;

const POPULAR_MIN_SCORE: i32 = 500;
const RISING_MAX_HOURS: u32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sort {
    Hot,
    New,
    Top,
    Rising,
}

impl Sort {
    pub(crate) const ALL: [Sort; 4] = [Sort::Hot, Sort::New, Sort::Top, Sort::Rising];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Sort::Hot => "Hot",
            Sort::New => "New",
            Sort::Top => "Top",
            Sort::Rising => "Rising",
        }
    }

    fn next(self) -> Sort {
        match self {
            Sort::Hot => Sort::New,
            Sort::New => Sort::Top,
            Sort::Top => Sort::Rising,
            Sort::Rising => Sort::Hot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Feed {
    Home,
    Popular,
    All,
    Saved,
    Group(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Feed,
    Search,
    Post { scroll: u16 },
}

/// A comment's position in a post's tree: its index among the top-level
/// comments, then among that comment's replies, and so on. The tree never
/// changes, so a path always names the same comment.
pub(crate) type CommentPath = Vec<usize>;

/// The open post's comment state. Reset whenever a post is opened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Thread {
    pub focus: Option<CommentPath>,
    pub collapsed: HashSet<CommentPath>,
    /// Set when `focus` moves. The renderer scrolls the focused comment into
    /// view once and clears it, so scrolling afterwards is free.
    pub reveal: bool,
}

/// A comment that isn't hidden under a collapsed ancestor.
pub(crate) struct Shown<'a> {
    pub path: CommentPath,
    pub comment: &'a Comment,
    pub collapsed: bool,
}

impl Shown<'_> {
    pub(crate) fn depth(&self) -> usize {
        self.path.len() - 1
    }
}

impl Thread {
    /// The comments of `comments` not hidden by a collapsed ancestor, in
    /// display order: a comment, then its replies, then its next sibling.
    /// This is the only walk over the tree; moving the focus and rendering
    /// both use it.
    pub(crate) fn visible<'a>(&self, comments: &'a [Comment]) -> Vec<Shown<'a>> {
        fn walk<'a>(
            thread: &Thread,
            comments: &'a [Comment],
            parent: &[usize],
            out: &mut Vec<Shown<'a>>,
        ) {
            for (i, comment) in comments.iter().enumerate() {
                let path = [parent, &[i]].concat();
                let collapsed = thread.collapsed.contains(&path);
                out.push(Shown {
                    path: path.clone(),
                    comment,
                    collapsed,
                });
                if !collapsed {
                    walk(thread, &comment.replies, &path, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self, comments, &[], &mut out);
        out
    }

    /// Moves the focus to the next (or previous) visible comment, stopping at
    /// the ends. With no focus, only forward picks the first comment.
    fn step(&mut self, comments: &[Comment], forward: bool) {
        let shown = self.visible(comments);
        let at = match &self.focus {
            None if forward => 0,
            None => return,
            Some(focus) => {
                let at = shown
                    .iter()
                    .position(|s| &s.path == focus)
                    .expect("the focused comment should always be visible");
                if forward {
                    at + 1
                } else if at == 0 {
                    return;
                } else {
                    at - 1
                }
            }
        };
        if let Some(next) = shown.into_iter().nth(at) {
            self.focus = Some(next.path);
            self.reveal = true;
        }
    }

    fn toggle_collapse(&mut self) {
        let Some(focus) = &self.focus else {
            return;
        };
        if !self.collapsed.remove(focus) {
            self.collapsed.insert(focus.clone());
        }
    }
}

/// Something on screen that a left click acts on. What it does depends on
/// the mode (see `App::on_mouse`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// A card, by index into `view`: any cell of its drawn rectangle.
    Card(usize),
    /// A card's `▲` (`Vote::Up`) or `▼` (`Vote::Down`).
    Vote(usize, Vote),
    /// A card's `☆ Save` or `★ Saved` label.
    Save(usize),
    /// A sort tab: its number and label.
    Sort(Sort),
    /// A nav entry.
    Feed(Feed),
    /// The header search box.
    Search,
    /// A comment's header row in the post view.
    Comment(CommentPath),
}

/// What the most recent frame drew where, written by the renderer on every
/// frame so that a click acts on what is on screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Hits {
    /// Clickable regions in drawing order. Where they overlap (a card's `▲`
    /// inside the card), the later one wins.
    pub regions: Vec<(Rect, Target)>,
    /// The post view's scroll clamp. 0 outside the post view.
    pub max_scroll: u16,
}

impl Hits {
    fn at(&self, column: u16, row: u16) -> Option<&Target> {
        let pos = Position::new(column, row);
        self.regions
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, target)| target)
    }
}

/// The comment at `path` in `comments` (see `CommentPath`).
pub(crate) fn comment_mut<'a>(
    comments: &'a mut [Comment],
    path: &[usize],
) -> Option<&'a mut Comment> {
    let (&first, rest) = path.split_first()?;
    let comment = comments.get_mut(first)?;
    if rest.is_empty() {
        Some(comment)
    } else {
        comment_mut(&mut comment.replies, rest)
    }
}

/// Applies `vote` to a post's or comment's `current` vote and `score`: ±1, or
/// ±2 when flipping, and repeating a vote clears it.
pub(crate) fn cast(current: &mut Vote, score: &mut i32, vote: Vote) {
    let value = |v: Vote| match v {
        Vote::Up => 1,
        Vote::None => 0,
        Vote::Down => -1,
    };
    let new = if *current == vote { Vote::None } else { vote };
    *score += value(new) - value(*current);
    *current = new;
}

pub(crate) struct App {
    pub groups: Vec<Group>,
    pub posts: Vec<Post>,
    /// Indices into `posts` for the current feed, filter and sort.
    pub view: Vec<usize>,
    pub selected: usize,
    /// Index into `view` of the first card drawn; maintained by the renderer.
    pub offset: usize,
    pub sort: Sort,
    pub feed: Feed,
    pub mode: Mode,
    pub query: String,
    /// `query` parsed; refreshed with `view` on every rebuild.
    pub search: Query,
    /// Comment focus and collapse state of the open post.
    pub thread: Thread,
    /// Clickable regions of the most recent frame; `None` until the first.
    pub hits: Option<Hits>,
    pub status: Option<&'static str>,
    pub karma: u32,
    pub should_quit: bool,
}

/// Reddit's "hot" ranking, with its 45000 second decay expressed in hours.
fn hot(post: &Post) -> f64 {
    let s = f64::from(post.score);
    s.signum() * s.abs().max(1.0).log10() - f64::from(post.hours_ago) / 12.5
}

fn rising(post: &Post) -> f64 {
    f64::from(post.score) / (f64::from(post.hours_ago) + 2.0)
}

/// Lowercases `text` and turns each whitespace run into one space. Each folded
/// char comes with the range of chars of `text` it stands for: one char, the
/// whole whitespace run, or the char that lowercased into several.
fn fold(text: &str) -> (Vec<char>, Vec<Range<usize>>) {
    let mut folded = Vec::new();
    let mut origin: Vec<Range<usize>> = Vec::new();
    for (i, ch) in text.chars().enumerate() {
        if ch.is_whitespace() {
            match origin.last_mut() {
                Some(run) if folded.last() == Some(&' ') && run.end == i => run.end = i + 1,
                _ => {
                    folded.push(' ');
                    origin.push(i..i + 1);
                }
            }
        } else {
            for lower in ch.to_lowercase() {
                folded.push(lower);
                origin.push(i..i + 1);
            }
        }
    }
    (folded, origin)
}

fn folded(text: &str) -> String {
    fold(text).0.into_iter().collect()
}

/// A parsed search query. Free-text terms must all match; filter values of the
/// same kind are alternatives. Every string is stored folded (see `fold`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Query {
    pub text: Vec<String>,
    pub groups: Vec<String>,
    pub authors: Vec<String>,
    pub flairs: Vec<String>,
}

impl Query {
    /// Splits on whitespace outside double quotes; an unclosed quote runs to
    /// the end. An unquoted term starting with `w/`, `u/` or `flair:` (in any
    /// case) is a filter on the rest of it. Terms and filter values that are
    /// empty or only whitespace (`""`, `" "`, `w/`) are dropped; others keep
    /// any leading or trailing space.
    pub(crate) fn parse(input: &str) -> Query {
        let mut terms: Vec<(String, bool)> = Vec::new();
        let mut cur: Option<(String, bool)> = None;
        let mut quoted = false;
        for ch in input.chars() {
            if ch.is_whitespace() && !quoted {
                terms.extend(cur.take());
                continue;
            }
            let (term, _) = cur.get_or_insert_with(|| (String::new(), ch == '"'));
            if ch == '"' {
                quoted = !quoted;
            } else {
                term.push(ch);
            }
        }
        terms.extend(cur);

        let mut query = Query::default();
        for (term, starts_quoted) in terms {
            let filter = [
                ("w/", &mut query.groups),
                ("u/", &mut query.authors),
                ("flair:", &mut query.flairs),
            ]
            .into_iter()
            .filter(|_| !starts_quoted)
            .find(|(prefix, _)| {
                term.get(..prefix.len())
                    .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
            });
            let (value, list) = match filter {
                Some((prefix, list)) => (&term[prefix.len()..], list),
                None => (term.as_str(), &mut query.text),
            };
            let value = folded(value);
            if value.chars().any(|ch| ch != ' ') {
                list.push(value);
            }
        }
        query
    }

    /// True when nothing is left to filter by, as for a blank query.
    pub(crate) fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.groups.is_empty()
            && self.authors.is_empty()
            && self.flairs.is_empty()
    }

    pub(crate) fn matches(&self, post: &Post, group: &str) -> bool {
        let starts_with = |values: &[String], field: Option<&str>| {
            values.is_empty()
                || field.is_some_and(|field| {
                    let field = folded(field);
                    values.iter().any(|v| field.starts_with(v.as_str()))
                })
        };
        if !(starts_with(&self.groups, Some(group))
            && starts_with(&self.authors, Some(post.author))
            && starts_with(&self.flairs, post.flair))
        {
            return false;
        }
        if self.text.is_empty() {
            return true;
        }
        let fields = [post.title, post.body, post.author, group].map(folded);
        self.text
            .iter()
            .all(|term| fields.iter().any(|field| field.contains(term.as_str())))
    }

    /// For each char of `text`, whether it is part of an occurrence of a
    /// free-text term. Overlapping occurrences simply mark the same chars.
    pub(crate) fn highlights(&self, text: &str) -> Vec<bool> {
        let mut marked = vec![false; text.chars().count()];
        if self.text.is_empty() {
            return marked;
        }
        let (folded, origin) = fold(text);
        for term in &self.text {
            let term: Vec<char> = term.chars().collect();
            for start in 0..(folded.len() + 1).saturating_sub(term.len()) {
                if folded[start..start + term.len()] == term[..] {
                    let chars = origin[start].start..origin[start + term.len() - 1].end;
                    marked[chars].fill(true);
                }
            }
        }
        marked
    }
}

impl App {
    pub(crate) fn new() -> Self {
        Self::with_data(data::groups(), data::posts())
    }

    pub(crate) fn with_data(groups: Vec<Group>, posts: Vec<Post>) -> Self {
        let mut app = App {
            groups,
            posts,
            view: Vec::new(),
            selected: 0,
            offset: 0,
            sort: Sort::Hot,
            feed: Feed::Home,
            mode: Mode::Feed,
            query: String::new(),
            search: Query::default(),
            thread: Thread::default(),
            hits: None,
            status: None,
            karma: 1_337,
            should_quit: false,
        };
        app.refresh();
        app
    }

    pub(crate) fn selected_post(&self) -> Option<&Post> {
        self.view.get(self.selected).map(|&i| &self.posts[i])
    }

    pub(crate) fn feed_name(&self) -> String {
        match self.feed {
            Feed::Home => "Home".to_owned(),
            Feed::Popular => "Popular".to_owned(),
            Feed::All => "All".to_owned(),
            Feed::Saved => "Saved".to_owned(),
            Feed::Group(g) => format!("w/{}", self.groups[g].name),
        }
    }

    fn in_feed(&self, post: &Post) -> bool {
        match self.feed {
            Feed::Home => self.groups[post.group].joined,
            Feed::Popular => post.score >= POPULAR_MIN_SCORE,
            Feed::All => true,
            Feed::Saved => post.saved,
            Feed::Group(g) => post.group == g,
        }
    }

    /// Rebuilds `view` and resets the selection to the top of the feed.
    pub(crate) fn refresh(&mut self) {
        self.search = Query::parse(&self.query);
        let mut view: Vec<usize> = (0..self.posts.len())
            .filter(|&i| {
                let post = &self.posts[i];
                self.in_feed(post)
                    && self.search.matches(post, self.groups[post.group].name)
                    && (self.sort != Sort::Rising || post.hours_ago < RISING_MAX_HOURS)
            })
            .collect();

        let posts = &self.posts;
        match self.sort {
            Sort::Hot => view.sort_by(|&a, &b| hot(&posts[b]).total_cmp(&hot(&posts[a]))),
            Sort::New => view.sort_by_key(|&i| posts[i].hours_ago),
            Sort::Top => view.sort_by_key(|&i| Reverse(posts[i].score)),
            Sort::Rising => view.sort_by(|&a, &b| rising(&posts[b]).total_cmp(&rising(&posts[a]))),
        }

        self.view = view;
        self.selected = 0;
        self.offset = 0;
    }

    fn set_sort(&mut self, sort: Sort) {
        self.sort = sort;
        self.refresh();
    }

    fn feeds(&self) -> Vec<Feed> {
        [Feed::Home, Feed::Popular, Feed::All, Feed::Saved]
            .into_iter()
            .chain((0..self.groups.len()).map(Feed::Group))
            .collect()
    }

    fn cycle_feed(&mut self, forward: bool) {
        let feeds = self.feeds();
        let n = feeds.len();
        let cur = feeds
            .iter()
            .position(|&f| f == self.feed)
            .expect("should always be on one of the listed feeds");
        self.feed = feeds[if forward {
            (cur + 1) % n
        } else {
            (cur + n - 1) % n
        }];
        self.refresh();
    }

    fn move_selection(&mut self, delta: isize) {
        let last = self.view.len().saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(last);
    }

    /// Votes on the selected post. Repeating a vote clears it, and the feed is
    /// deliberately not re-sorted so the card stays under the cursor.
    fn vote(&mut self, vote: Vote) {
        let Some(&i) = self.view.get(self.selected) else {
            return;
        };
        let post = &mut self.posts[i];
        cast(&mut post.vote, &mut post.score, vote);
    }

    /// Votes on the open post's comment at `path`. Comments are never
    /// reordered.
    fn vote_comment(&mut self, path: &[usize], vote: Vote) {
        let Some(&i) = self.view.get(self.selected) else {
            return;
        };
        if let Some(comment) = comment_mut(&mut self.posts[i].comments, path) {
            cast(&mut comment.vote, &mut comment.score, vote);
        }
    }

    fn open_post(&mut self) {
        self.thread = Thread::default();
        self.mode = Mode::Post { scroll: 0 };
    }

    fn step_focus(&mut self, forward: bool) {
        let Some(&i) = self.view.get(self.selected) else {
            return;
        };
        self.thread.step(&self.posts[i].comments, forward);
    }

    fn toggle_save(&mut self) {
        let Some(&i) = self.view.get(self.selected) else {
            return;
        };
        let post = &mut self.posts[i];
        post.saved = !post.saved;
        self.status = Some(if post.saved {
            "Post saved"
        } else {
            "Post unsaved"
        });
    }

    /// Joins or leaves the open group. The group feed doesn't depend on
    /// membership, so it isn't rebuilt; Home picks up the change on its next
    /// rebuild.
    fn toggle_join(&mut self) {
        let Feed::Group(g) = self.feed else {
            return;
        };
        let group = &mut self.groups[g];
        group.joined = !group.joined;
        self.status = Some(if group.joined {
            "Joined group"
        } else {
            "Left group"
        });
    }

    pub(crate) fn on_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        self.status = None;
        match self.mode {
            Mode::Feed => self.on_feed_key(key.code),
            Mode::Search => self.on_search_key(key.code),
            Mode::Post { scroll } => self.on_post_key(key.code, scroll),
        }
    }

    /// Whether `on_mouse` acts on `event` at all: a left press or the wheel,
    /// after the first frame, outside search mode. The rest change nothing,
    /// so the event loop doesn't redraw for them.
    pub(crate) fn handles_mouse(&self, event: &MouseEvent) -> bool {
        self.hits.is_some()
            && self.mode != Mode::Search
            && matches!(
                event.kind,
                MouseEventKind::Down(MouseButton::Left)
                    | MouseEventKind::ScrollDown
                    | MouseEventKind::ScrollUp
            )
    }

    /// Left presses act on whatever the last frame drew under the pointer, and
    /// the wheel moves like `j`/`k`. Events `handles_mouse` rejects do nothing
    /// at all, not even clear the toast.
    pub(crate) fn on_mouse(&mut self, event: MouseEvent) {
        if !self.handles_mouse(&event) {
            return;
        }
        let Some(hits) = &self.hits else {
            return;
        };
        let wheel = match event.kind {
            MouseEventKind::ScrollDown => Some(1),
            MouseEventKind::ScrollUp => Some(-1),
            _ => None,
        };
        let target = hits.at(event.column, event.row).cloned();
        let max_scroll = hits.max_scroll;
        self.status = None;
        match (self.mode, wheel) {
            (Mode::Feed, Some(delta)) => self.move_selection(delta),
            (Mode::Feed, None) => {
                if let Some(target) = target {
                    self.on_feed_click(target);
                }
            }
            (Mode::Post { scroll }, Some(delta)) => {
                // 3 lines per notch, clamped as the last frame drew it.
                let scroll = if delta > 0 {
                    scroll.saturating_add(3).min(max_scroll.max(scroll))
                } else {
                    scroll.saturating_sub(3)
                };
                self.mode = Mode::Post { scroll };
            }
            (Mode::Post { .. }, None) => {
                if let Some(Target::Comment(path)) = target {
                    self.click_comment(path);
                }
            }
            (Mode::Search, _) => {}
        }
    }

    fn on_feed_click(&mut self, target: Target) {
        match target {
            Target::Card(i) if i < self.view.len() => {
                self.selected = i;
                self.open_post();
            }
            Target::Vote(i, vote) if i < self.view.len() => {
                self.selected = i;
                self.vote(vote);
            }
            Target::Save(i) if i < self.view.len() => {
                self.selected = i;
                self.toggle_save();
            }
            Target::Sort(sort) => self.set_sort(sort),
            Target::Feed(feed) if feed != self.feed => {
                self.feed = feed;
                self.refresh();
            }
            Target::Search => self.mode = Mode::Search,
            _ => {}
        }
    }

    /// Focuses the comment, or toggles its collapse when it already has the
    /// focus. Never scrolls: unlike `n`/`p`, it asks for no reveal, since the
    /// header is on screen where it was clicked.
    fn click_comment(&mut self, path: CommentPath) {
        let Some(&i) = self.view.get(self.selected) else {
            return;
        };
        if self.thread.focus.as_ref() == Some(&path) {
            self.thread.toggle_collapse();
        } else if self
            .thread
            .visible(&self.posts[i].comments)
            .iter()
            .any(|s| s.path == path)
        {
            self.thread.focus = Some(path);
            self.thread.reveal = false;
        }
    }

    fn on_feed_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc if !self.query.is_empty() => {
                self.query.clear();
                self.refresh();
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::PageDown => self.move_selection(5),
            KeyCode::PageUp => self.move_selection(-5),
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.move_selection(isize::MAX),
            KeyCode::Char('a') => self.vote(Vote::Up),
            KeyCode::Char('z') => self.vote(Vote::Down),
            KeyCode::Char('s') => self.toggle_save(),
            KeyCode::Char('J') => self.toggle_join(),
            KeyCode::Char('c') => self.status = Some("Link copied to clipboard!"),
            KeyCode::Enter if !self.view.is_empty() => self.open_post(),
            KeyCode::Char(ch @ '1'..='4') => self.set_sort(Sort::ALL[usize::from(ch as u8 - b'1')]),
            KeyCode::Tab => self.set_sort(self.sort.next()),
            KeyCode::Char(']') => self.cycle_feed(true),
            KeyCode::Char('[') => self.cycle_feed(false),
            KeyCode::Char('/') => self.mode = Mode::Search,
            _ => {}
        }
    }

    fn on_search_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Enter => self.mode = Mode::Feed,
            KeyCode::Esc => {
                self.mode = Mode::Feed;
                self.query.clear();
                self.refresh();
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.refresh();
            }
            KeyCode::Char(ch) => {
                self.query.push(ch);
                self.refresh();
            }
            _ => {}
        }
    }

    /// The renderer clamps `scroll` to the content height, so scrolling past
    /// the end here is harmless. It also scrolls a newly focused comment into
    /// view.
    fn on_post_key(&mut self, code: KeyCode, scroll: u16) {
        let focus = self.thread.focus.clone();
        let scroll = match code {
            KeyCode::Esc if focus.is_some() => {
                self.thread.focus = None;
                scroll
            }
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Backspace => {
                self.mode = Mode::Feed;
                return;
            }
            KeyCode::Char('j') | KeyCode::Down => scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => scroll.saturating_sub(1),
            KeyCode::PageDown | KeyCode::Char(' ') => scroll.saturating_add(10),
            KeyCode::PageUp => scroll.saturating_sub(10),
            KeyCode::Char('g') | KeyCode::Home => 0,
            KeyCode::Char('G') | KeyCode::End => u16::MAX,
            KeyCode::Char('n') => {
                self.step_focus(true);
                scroll
            }
            KeyCode::Char('p') => {
                self.step_focus(false);
                scroll
            }
            KeyCode::Enter => {
                self.thread.toggle_collapse();
                scroll
            }
            KeyCode::Char(key @ ('a' | 'z')) => {
                let vote = if key == 'a' { Vote::Up } else { Vote::Down };
                match focus {
                    Some(path) => self.vote_comment(&path, vote),
                    None => self.vote(vote),
                }
                scroll
            }
            KeyCode::Char('s') => {
                self.toggle_save();
                scroll
            }
            KeyCode::Char('c') => {
                self.status = Some("Link copied to clipboard!");
                scroll
            }
            _ => scroll,
        };
        self.mode = Mode::Post { scroll };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn post(group: usize, score: i32, hours_ago: u32, title: &'static str) -> Post {
        Post {
            group,
            score,
            hours_ago,
            title,
            ..Default::default()
        }
    }

    fn group(name: &'static str, joined: bool) -> Group {
        Group {
            name,
            description: "",
            members: 0,
            online: 0,
            joined,
        }
    }

    fn sample() -> App {
        App::with_data(
            vec![group("Joined", true), group("Other", false)],
            vec![
                post(0, 10, 1, "fresh small"),
                post(0, 5_000, 30, "old huge"),
                post(1, 900, 2, "other group"),
                post(0, 600, 5, "mid"),
            ],
        )
    }

    fn titles(app: &App) -> Vec<&'static str> {
        app.view.iter().map(|&i| app.posts[i].title).collect()
    }

    #[test]
    fn home_feed_only_shows_joined_groups() {
        let app = sample();
        assert!(
            !titles(&app).contains(&"other group"),
            "group 1 is not joined, so Home must hide it"
        );
        assert_eq!(app.view.len(), 3);
    }

    #[test]
    fn sorts() {
        let mut app = sample();
        press(&mut app, KeyCode::Char(']'));
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.feed, Feed::All);

        press(&mut app, KeyCode::Char('2'));
        assert_eq!(
            titles(&app),
            ["fresh small", "other group", "mid", "old huge"],
            "New orders by age"
        );

        press(&mut app, KeyCode::Char('3'));
        assert_eq!(
            titles(&app),
            ["old huge", "other group", "mid", "fresh small"],
            "Top orders by score"
        );

        // Rising drops "old huge" (30h old); 900/4 = 225 beats 600/7 ≈ 86 and 10/3 ≈ 3.
        press(&mut app, KeyCode::Char('4'));
        assert_eq!(titles(&app), ["other group", "mid", "fresh small"]);

        // Hot: log10(900) - 2/12.5 ≈ 2.79 beats log10(600) - 5/12.5 ≈ 2.38 and
        // log10(5000) - 30/12.5 ≈ 1.30; age decay sinks the high scorer.
        press(&mut app, KeyCode::Char('1'));
        assert_eq!(
            titles(&app),
            ["other group", "mid", "old huge", "fresh small"]
        );
    }

    #[test]
    fn popular_feed_hides_low_scores() {
        let mut app = sample();
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.feed, Feed::Popular);
        assert!(app.view.iter().all(|&i| app.posts[i].score >= 500));
        assert_eq!(app.view.len(), 3);
    }

    #[test]
    fn feed_cycle_wraps_both_ways() {
        let mut app = sample();
        press(&mut app, KeyCode::Char('['));
        assert_eq!(app.feed, Feed::Group(1));
        assert_eq!(titles(&app), ["other group"]);
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.feed, Feed::Home);
    }

    #[test]
    fn voting_toggles_and_flips() {
        let mut app = sample();
        let i = app.view[0];
        let base = app.posts[i].score;

        press(&mut app, KeyCode::Char('a'));
        assert_eq!(app.posts[i].score, base + 1);
        press(&mut app, KeyCode::Char('z'));
        assert_eq!(app.posts[i].score, base - 1, "flipping up to down is -2");
        assert_eq!(app.posts[i].vote, Vote::Down);
        press(&mut app, KeyCode::Char('z'));
        assert_eq!(app.posts[i].score, base, "repeating a vote clears it");
        assert_eq!(app.posts[i].vote, Vote::None);
        assert_eq!(app.view[0], i, "voting must not reorder the feed");
    }

    #[test]
    fn selection_is_clamped() {
        let mut app = sample();
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.selected, 0);
        press(&mut app, KeyCode::Char('G'));
        assert_eq!(app.selected, app.view.len() - 1);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.selected, app.view.len() - 1);
    }

    #[test]
    fn search_filters_live_and_esc_clears() {
        let mut app = sample();
        press(&mut app, KeyCode::Char('/'));
        "HUGE"
            .chars()
            .for_each(|ch| press(&mut app, KeyCode::Char(ch)));
        assert_eq!(app.mode, Mode::Search);
        assert_eq!(titles(&app), ["old huge"], "search is case-insensitive");

        press(&mut app, KeyCode::Char('q'));
        assert!(app.view.is_empty(), "q is typed, not quit, while searching");
        assert!(!app.should_quit);

        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, Mode::Feed);
        assert!(app.query.is_empty());
        assert_eq!(app.view.len(), 3);
    }

    #[test]
    fn esc_in_feed_clears_kept_search_without_quitting() {
        let mut app = sample();
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('m'));
        press(&mut app, KeyCode::Char('i'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(titles(&app), ["mid"], "Enter keeps the filter");

        press(&mut app, KeyCode::Esc);
        assert!(app.query.is_empty());
        assert_eq!(app.view.len(), 3);
        press(&mut app, KeyCode::Esc);
        assert!(!app.should_quit, "only q quits from the feed");
        press(&mut app, KeyCode::Char('q'));
        assert!(app.should_quit);
    }

    #[test]
    fn enter_opens_post_and_esc_returns() {
        let mut app = sample();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.mode, Mode::Post { scroll: 1 });
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, Mode::Feed);
        assert!(
            !app.should_quit,
            "Esc in a post goes back instead of quitting"
        );
    }

    #[test]
    fn enter_on_empty_feed_does_nothing() {
        let mut app = sample();
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('#'));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, Mode::Feed);
    }

    fn press_str(app: &mut App, text: &str) {
        text.chars().for_each(|ch| press(app, KeyCode::Char(ch)));
    }

    #[test]
    fn joining_and_leaving_groups_updates_home() {
        let mut app = sample();
        press(&mut app, KeyCode::Char('['));
        assert_eq!(app.feed, Feed::Group(1));
        press(&mut app, KeyCode::Char('J'));
        assert!(app.groups[1].joined);
        assert_eq!(app.status, Some("Joined group"));
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.feed, Feed::Home);
        assert_eq!(
            titles(&app),
            ["other group", "mid", "old huge", "fresh small"],
            "Home now includes the joined group, in Hot order"
        );

        press(&mut app, KeyCode::Char('['));
        press(&mut app, KeyCode::Char('['));
        assert_eq!(app.feed, Feed::Group(0));
        press(&mut app, KeyCode::Char('J'));
        assert!(!app.groups[0].joined);
        assert_eq!(app.status, Some("Left group"));
        press(&mut app, KeyCode::Char(']'));
        press(&mut app, KeyCode::Char(']'));
        assert_eq!(app.feed, Feed::Home);
        assert_eq!(
            titles(&app),
            ["other group"],
            "Home hides the group that was left"
        );
    }

    #[test]
    fn join_on_group_feed_keeps_view_and_selection() {
        let mut app = sample();
        press_str(&mut app, "]]]]");
        assert_eq!(app.feed, Feed::Group(0));
        press(&mut app, KeyCode::Char('j'));
        let view = app.view.clone();

        press(&mut app, KeyCode::Char('J'));
        assert!(!app.groups[0].joined);
        assert_eq!(app.view, view, "the group feed must not be rebuilt");
        assert_eq!(app.selected, 1);

        press(&mut app, KeyCode::Char('J'));
        assert!(app.groups[0].joined);
        assert_eq!(app.status, Some("Joined group"));
        assert_eq!(app.view, view);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn join_does_nothing_outside_group_feeds_and_is_typed_in_search() {
        let mut app = sample();
        for feed in [Feed::Home, Feed::Popular, Feed::All, Feed::Saved] {
            assert_eq!(app.feed, feed);
            press(&mut app, KeyCode::Char('J'));
            assert!(app.groups[0].joined && !app.groups[1].joined, "{feed:?}");
            assert_eq!(app.status, None, "no toast on {feed:?}");
            press(&mut app, KeyCode::Char(']'));
        }

        assert_eq!(app.feed, Feed::Group(0));
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('J'));
        assert_eq!(app.query, "J", "J is typed while searching");
        assert!(app.groups[0].joined, "and doesn't leave the group");
        assert_eq!(app.status, None);
    }

    #[test]
    fn saved_feed_lists_saved_posts_with_search_and_sort() {
        let mut app = sample();
        [1, 2, 3].iter().for_each(|&i| app.posts[i].saved = true);
        press_str(&mut app, "]]]");
        assert_eq!(app.feed, Feed::Saved);
        assert_eq!(
            titles(&app),
            ["other group", "mid", "old huge"],
            "saved posts from any group, joined or not, in Hot order"
        );

        press(&mut app, KeyCode::Char('4'));
        assert_eq!(
            titles(&app),
            ["other group", "mid"],
            "Rising drops the 30h old post"
        );

        press(&mut app, KeyCode::Char('/'));
        press_str(&mut app, "mi");
        assert_eq!(titles(&app), ["mid"]);
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Char('/'));
        press_str(&mut app, "fresh");
        assert!(app.view.is_empty(), "\"fresh small\" isn't saved");
    }

    #[test]
    fn unsaving_in_saved_feed_keeps_card_until_rebuild() {
        let mut app = sample();
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.status, Some("Post saved"));
        press_str(&mut app, "]]]");
        assert_eq!(titles(&app), ["mid"]);

        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.status, Some("Post unsaved"));
        assert!(!app.posts[3].saved);
        assert_eq!(titles(&app), ["mid"], "the card stays until a rebuild");
        assert_eq!(app.selected, 0);

        press(&mut app, KeyCode::Char('1'));
        assert!(app.view.is_empty(), "gone after the rebuild");
    }

    #[test]
    fn unsaving_from_post_view_in_saved_feed_keeps_card_until_rebuild() {
        let mut app = sample();
        app.posts[1].saved = true;
        app.posts[3].saved = true;
        press_str(&mut app, "]]]");
        assert_eq!(titles(&app), ["mid", "old huge"]);
        press(&mut app, KeyCode::Char('j'));
        let view = app.view.clone();

        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('s'));
        assert_eq!(app.status, Some("Post unsaved"));
        assert!(!app.posts[1].saved);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, Mode::Feed);
        assert_eq!(app.view, view, "the card stays until a rebuild");
        assert_eq!(app.selected, 1);

        press(&mut app, KeyCode::Char('2'));
        assert_eq!(titles(&app), ["mid"], "gone after the rebuild");
    }

    #[test]
    fn join_with_kept_search_keeps_query_view_and_selection() {
        let mut app = sample();
        press_str(&mut app, "]]]]");
        assert_eq!(app.feed, Feed::Group(0));
        press(&mut app, KeyCode::Char('/'));
        press(&mut app, KeyCode::Char('m'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(titles(&app), ["mid", "fresh small"]);
        press(&mut app, KeyCode::Char('j'));
        let view = app.view.clone();

        press(&mut app, KeyCode::Char('J'));
        assert!(!app.groups[0].joined);
        assert_eq!(app.status, Some("Left group"));
        assert_eq!(app.mode, Mode::Feed);
        assert_eq!(app.query, "m");
        assert_eq!(app.view, view);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn join_does_nothing_in_post_view() {
        let mut app = sample();
        press_str(&mut app, "]]]]");
        assert_eq!(app.feed, Feed::Group(0));
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('J'));
        assert!(app.groups[0].joined, "J only works in the feed");
        assert_eq!(app.status, None);
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
    }

    #[test]
    fn feed_cycle_includes_saved_after_all() {
        let mut app = sample();
        let mut cycle = |key| {
            (0..6)
                .map(|_| {
                    press(&mut app, KeyCode::Char(key));
                    app.feed
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            cycle(']'),
            [
                Feed::Popular,
                Feed::All,
                Feed::Saved,
                Feed::Group(0),
                Feed::Group(1),
                Feed::Home
            ]
        );
        assert_eq!(
            cycle('['),
            [
                Feed::Group(1),
                Feed::Group(0),
                Feed::Saved,
                Feed::All,
                Feed::Popular,
                Feed::Home
            ]
        );
    }

    #[test]
    fn query_tokenizer() {
        let q = Query::parse("  Dead \t CODE  ");
        assert_eq!(q.text, ["dead", "code"], "whitespace runs split terms");
        assert_eq!(
            q,
            Query {
                text: vec!["dead".into(), "code".into()],
                ..Query::default()
            }
        );

        let q = Query::parse(r#"a "Dead   code" b"#);
        assert_eq!(q.text, ["a", "dead code", "b"], "a phrase is one term");
        assert_eq!(
            Query::parse(r#"x "dead co"#).text,
            ["x", "dead co"],
            "an unclosed quote runs to the end"
        );
        assert_eq!(Query::parse(r#""" a """#).text, ["a"], "\"\" is ignored");

        let q = Query::parse(r#"W/Eng u/Dana FLAIR:"looking for" Flair:win"#);
        assert_eq!(q.groups, ["eng"]);
        assert_eq!(q.authors, ["dana"]);
        assert_eq!(q.flairs, ["looking for", "win"]);
        assert!(q.text.is_empty(), "filters aren't free text: {q:?}");

        let q = Query::parse(r#""w/eng" "U/x y""#);
        assert_eq!(
            q.text,
            ["w/eng", "u/x y"],
            "a quoted term is always free text"
        );
        assert!(q.groups.is_empty() && q.authors.is_empty());

        assert_eq!(
            Query::parse(r#"w/ u/ flair: flair:"" W/ FLAIR:"#),
            Query::default(),
            "filters with empty values are ignored"
        );
    }

    fn search_sample() -> App {
        let p = |group, author, flair, title, body| Post {
            group,
            author,
            flair,
            title,
            body,
            score: 100,
            hours_ago: 1,
            ..Default::default()
        };
        App::with_data(
            vec![
                group("Engineering", true),
                group("OfficeLife", true),
                group("OncallSurvivors", true),
                group("Foodies", true),
            ],
            vec![
                p(
                    0,
                    "dana_k",
                    Some("Win"),
                    "We deleted 40k lines of dead code",
                    "The build got 30% faster.\nNext: tests.",
                ),
                p(
                    1,
                    "priya_r",
                    Some("PSA"),
                    "Espresso machine is fixed",
                    "Facilities replaced the grinder.",
                ),
                p(
                    2,
                    "dana_k",
                    None,
                    "Paged at 3am",
                    "A dashboard had a typo in its alert threshold.",
                ),
                p(
                    3,
                    "sam_o",
                    Some("Looking for team"),
                    "Pizza ranking",
                    "Pineapple jalapeño was brave.",
                ),
            ],
        )
    }

    const DEAD: &str = "We deleted 40k lines of dead code";
    const ESPRESSO: &str = "Espresso machine is fixed";
    const PAGED: &str = "Paged at 3am";
    const PIZZA: &str = "Pizza ranking";

    /// Types `query` live into a fresh `search_sample` and returns the titles.
    fn searched(query: &str) -> Vec<&'static str> {
        let mut app = search_sample();
        assert_eq!(titles(&app), [DEAD, ESPRESSO, PAGED, PIZZA]);
        press(&mut app, KeyCode::Char('/'));
        press_str(&mut app, query);
        assert_eq!(app.search, Query::parse(query), "parsed on every rebuild");
        assert_eq!(app.selected, 0);
        titles(&app)
    }

    #[test]
    fn free_text_terms_and_together() {
        assert_eq!(searched("dead faster"), [DEAD], "title and body words");
        assert_eq!(searched("FASTER Dead"), [DEAD], "in any order and case");
        assert_eq!(
            searched("dead zebra"),
            [] as [&str; 0],
            "every term must match"
        );
        assert_eq!(searched("dana"), [DEAD, PAGED], "author is free text too");
        assert_eq!(searched("engineering"), [DEAD], "and so is the group");
        assert_eq!(
            searched(r#""dead faster""#),
            [] as [&str; 0],
            "a phrase needs its words together"
        );
    }

    #[test]
    fn phrase_matches_across_newline_in_body() {
        assert_eq!(searched(r#""faster. next""#), [DEAD]);
        assert_eq!(
            searched(r#""faster.   NEXT: tests"#),
            [DEAD],
            "unclosed, extra spaces"
        );
    }

    #[test]
    fn filters_match_prefixes_or_within_a_kind_and_across_kinds() {
        assert_eq!(
            searched("w/o"),
            [ESPRESSO, PAGED],
            "OfficeLife and OncallSurvivors"
        );
        assert_eq!(searched("W/ONCALL"), [PAGED]);
        assert_eq!(searched("w/eng w/food"), [DEAD, PIZZA], "same kind ORs");
        assert_eq!(searched("u/dana"), [DEAD, PAGED]);
        assert_eq!(searched("w/eng u/dana"), [DEAD], "different kinds AND");
        assert_eq!(searched("w/eng u/priya"), [] as [&str; 0]);
        assert_eq!(searched("w/office facilities"), [ESPRESSO]);
        assert_eq!(
            searched("w/office dead"),
            [] as [&str; 0],
            "free text ANDs too"
        );
        assert_eq!(searched("w/engineer"), [DEAD]);
        assert_eq!(
            searched("w/ngineering"),
            [] as [&str; 0],
            "prefix, not substring"
        );
    }

    #[test]
    fn flair_filter_never_matches_posts_without_flair() {
        assert_eq!(searched(r#"flair:"looking for""#), [PIZZA]);
        assert_eq!(searched("flair:ps flair:WIN"), [DEAD, ESPRESSO]);
        assert_eq!(searched("u/dana flair:w"), [DEAD], "PAGED has no flair");
        assert_eq!(
            searched("flair:team"),
            [] as [&str; 0],
            "prefix, not substring"
        );
    }

    #[test]
    fn blank_query_and_empty_filters_match_every_post() {
        let all = [DEAD, ESPRESSO, PAGED, PIZZA];
        for query in ["   ", "w/", "u/ flair:", r#"flair:"""#, r#""""#] {
            assert_eq!(searched(query), all, "{query:?}");
        }
        let mut app = search_sample();
        press_str(&mut app, "]]]]");
        assert_eq!(app.feed, Feed::Group(0));
        press(&mut app, KeyCode::Char('/'));
        press_str(&mut app, "  ");
        assert_eq!(titles(&app), [DEAD], "every post in the feed, not more");
    }

    #[test]
    fn whitespace_only_phrases_are_ignored() {
        assert_eq!(
            Query::parse("\" \" \"   \" \"\t \" flair:\" \" w/\"  "),
            Query::default(),
            "a phrase or value of only whitespace is no term"
        );
        assert!(Query::parse("\"  \"").is_empty());
        assert_eq!(
            Query::parse("\"dead \"").text,
            ["dead "],
            "real text keeps its space"
        );
        assert_eq!(Query::parse("\" dead  \"").text, [" dead "]);

        let all = [DEAD, ESPRESSO, PAGED, PIZZA];
        assert_eq!(searched("\" \""), all);
        assert_eq!(searched("\" \"   \"   \" w/"), all);
        assert_eq!(searched("\"dead \""), [DEAD]);
        assert_eq!(
            searched("\"code \""),
            [] as [&str; 0],
            "the title ends in \"code\", with no space after it"
        );
        assert!(
            !Query::parse("\" \"").highlights("a b  c").contains(&true),
            "and nothing is highlighted"
        );
    }

    #[test]
    fn highlights_mark_chars_of_every_term() {
        let marks = |query: &str, text: &str| -> String {
            Query::parse(query)
                .highlights(text)
                .iter()
                .map(|&m| if m { '^' } else { '.' })
                .collect()
        };
        assert_eq!(marks("ANA", "Banana"), ".^^^^^", "overlapping occurrences");
        assert_eq!(marks(r#"dead "ad co""#, "dead code"), "^^^^^^^..");
        assert_eq!(marks("dead code", "dead  code"), "^^^^..^^^^");
        assert_eq!(marks(r#""dead code""#, "dead \n code"), "^^^^^^^^^^^");
        assert_eq!(
            marks("w/dead u/dead", "dead"),
            "....",
            "filters aren't marked"
        );
        assert_eq!(marks("°f", "64°F!"), "..^^.");
        assert_eq!(marks("jalapeño", "a jalapeño"), "..^^^^^^^^");
        assert_eq!(
            marks("İs", "xİstanbul"),
            ".^^......",
            "İ lowercases to 2 chars"
        );
        assert_eq!(marks("i̇ i", "İİ"), "^^");
        assert_eq!(marks("x", ""), "");
    }

    /// One post whose comments, in display order, are a, b, c, d, e, f, g:
    ///
    /// ```text
    /// a
    /// │ b
    /// │ │ c
    /// │ │ │ d
    /// │ e
    /// f
    /// │ g
    /// ```
    fn thread_sample() -> App {
        let c = |author, score| data::c(author, score, 1, "");
        let mut thread = post(0, 50, 1, "thread");
        thread.comments = vec![
            c("a", 10).with(vec![
                c("b", 20).with(vec![c("c", 30).with(vec![c("d", 40)])]),
                c("e", 50),
            ]),
            c("f", 60).with(vec![c("g", 70)]),
        ];
        App::with_data(vec![group("Joined", true)], vec![thread])
    }

    fn focused(app: &App) -> Option<&'static str> {
        let focus = app.thread.focus.as_ref()?;
        let comments = &app.posts[app.view[app.selected]].comments;
        let shown = app.thread.visible(comments);
        Some(shown.iter().find(|s| &s.path == focus)?.comment.author)
    }

    fn visible_authors(app: &App) -> Vec<&'static str> {
        let comments = &app.posts[app.view[app.selected]].comments;
        app.thread
            .visible(comments)
            .iter()
            .map(|s| s.comment.author)
            .collect()
    }

    #[test]
    fn comment_keys_on_post_without_comments_do_nothing() {
        let mut app = sample();
        press(&mut app, KeyCode::Enter);
        assert!(app.posts[app.view[0]].comments.is_empty());
        for key in [KeyCode::Char('n'), KeyCode::Char('p'), KeyCode::Enter] {
            press(&mut app, key);
            assert_eq!(app.thread, Thread::default(), "{key:?}");
            assert_eq!(app.mode, Mode::Post { scroll: 0 });
        }
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(
            app.posts[app.view[0]].vote,
            Vote::Up,
            "a still votes the post"
        );
    }

    #[test]
    fn focus_walks_comments_in_pre_order_and_stops_at_both_ends() {
        let mut app = thread_sample();
        press(&mut app, KeyCode::Enter);
        assert_eq!(focused(&app), None, "nothing is focused on open");
        press(&mut app, KeyCode::Char('p'));
        assert_eq!(focused(&app), None, "p with no focus does nothing");
        assert!(!app.thread.reveal);

        let mut order = Vec::new();
        for _ in 0..7 {
            press(&mut app, KeyCode::Char('n'));
            assert!(app.thread.reveal, "moving the focus asks for a reveal");
            app.thread.reveal = false;
            order.extend(focused(&app));
        }
        assert_eq!(order, ["a", "b", "c", "d", "e", "f", "g"]);
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(focused(&app), Some("g"), "n stops at the last comment");
        assert!(!app.thread.reveal, "and doesn't count as a move");

        let mut order = Vec::new();
        for _ in 0..6 {
            press(&mut app, KeyCode::Char('p'));
            order.extend(focused(&app));
        }
        assert_eq!(order, ["f", "e", "d", "c", "b", "a"]);
        app.thread.reveal = false;
        press(&mut app, KeyCode::Char('p'));
        assert_eq!(focused(&app), Some("a"), "p stops at the first comment");
        assert!(!app.thread.reveal);
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
    }

    #[test]
    fn focus_skips_collapsed_replies_and_each_comment_keeps_its_state() {
        let mut app = thread_sample();
        press(&mut app, KeyCode::Enter);
        press_str(&mut app, "nn");
        assert_eq!(focused(&app), Some("b"));
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            focused(&app),
            Some("b"),
            "focus stays on a collapsed comment"
        );
        assert_eq!(visible_authors(&app), ["a", "b", "e", "f", "g"]);
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(focused(&app), Some("e"), "c and d are skipped");
        press(&mut app, KeyCode::Char('p'));
        assert_eq!(focused(&app), Some("b"));

        press(&mut app, KeyCode::Char('p'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(visible_authors(&app), ["a", "f", "g"]);
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(focused(&app), Some("f"), "everything under a is skipped");
        press(&mut app, KeyCode::Char('p'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(
            visible_authors(&app),
            ["a", "b", "e", "f", "g"],
            "b is still collapsed after expanding a"
        );
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(focused(&app), Some("b"));
        assert_eq!(visible_authors(&app), ["a", "b", "c", "d", "e", "f", "g"]);
    }

    #[test]
    fn comment_votes_toggle_and_flip_without_touching_the_post() {
        let mut app = thread_sample();
        press(&mut app, KeyCode::Enter);
        press_str(&mut app, "nnn");
        assert_eq!(focused(&app), Some("c"));
        let comment = |app: &App| {
            let c = &app.posts[0].comments[0].replies[0].replies[0];
            (c.score, c.vote)
        };
        let order = visible_authors(&app);

        press(&mut app, KeyCode::Char('a'));
        assert_eq!(comment(&app), (31, Vote::Up));
        press(&mut app, KeyCode::Char('z'));
        assert_eq!(comment(&app), (29, Vote::Down), "flipping is -2");
        press(&mut app, KeyCode::Char('z'));
        assert_eq!(comment(&app), (30, Vote::None), "repeating clears it");
        press(&mut app, KeyCode::Char('z'));
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(comment(&app), (31, Vote::Up), "flipping is +2");

        assert_eq!((app.posts[0].score, app.posts[0].vote), (50, Vote::None));
        assert_eq!(visible_authors(&app), order, "votes never reorder comments");
        assert_eq!(app.posts[0].comments[0].replies[1].vote, Vote::None);
    }

    #[test]
    fn comment_votes_survive_reopening_but_focus_and_collapse_reset() {
        let mut app = thread_sample();
        press(&mut app, KeyCode::Enter);
        press_str(&mut app, "na");
        press(&mut app, KeyCode::Enter);
        assert_eq!(visible_authors(&app), ["a", "f", "g"]);
        press(&mut app, KeyCode::Char('q'));
        assert_eq!(app.mode, Mode::Feed);

        press(&mut app, KeyCode::Enter);
        assert_eq!(app.thread, Thread::default(), "focus and collapse reset");
        assert_eq!(visible_authors(&app), ["a", "b", "c", "d", "e", "f", "g"]);
        let a = &app.posts[0].comments[0];
        assert_eq!((a.score, a.vote), (11, Vote::Up), "the vote is kept");
    }

    #[test]
    fn esc_unfocuses_before_leaving_and_q_leaves_directly() {
        let mut app = thread_sample();
        press(&mut app, KeyCode::Enter);
        press_str(&mut app, "jn");
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.thread.focus, None);
        assert_eq!(app.mode, Mode::Post { scroll: 1 }, "still in the post");
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, Mode::Feed);

        for leave in [KeyCode::Char('q'), KeyCode::Backspace] {
            press(&mut app, KeyCode::Enter);
            press(&mut app, KeyCode::Char('n'));
            assert!(app.thread.focus.is_some());
            press(&mut app, leave);
            assert_eq!(app.mode, Mode::Feed, "{leave:?}");
            assert!(!app.should_quit);
        }
    }

    #[test]
    fn save_and_share_act_on_the_post_and_enter_needs_focus() {
        let mut app = thread_sample();
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.thread, Thread::default(), "Enter with no focus");
        assert_eq!(app.mode, Mode::Post { scroll: 0 });

        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Char('s'));
        assert!(app.posts[0].saved);
        assert_eq!(app.status, Some("Post saved"));
        press(&mut app, KeyCode::Char('c'));
        assert_eq!(app.status, Some("Link copied to clipboard!"));
        assert_eq!(focused(&app), Some("a"), "still focused");
        assert!(app.thread.collapsed.is_empty());
    }

    #[test]
    fn comment_keys_mean_nothing_in_the_feed_and_are_typed_in_search() {
        let mut app = thread_sample();
        press_str(&mut app, "np");
        assert_eq!(app.mode, Mode::Feed);
        assert_eq!((app.view.clone(), app.selected), (vec![0], 0));
        assert_eq!(app.thread, Thread::default());

        press(&mut app, KeyCode::Char('/'));
        press_str(&mut app, "np");
        assert_eq!(app.query, "np");
        assert_eq!(app.mode, Mode::Search);
    }

    fn mouse(app: &mut App, kind: MouseEventKind, column: u16, row: u16) {
        app.on_mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        });
    }

    #[test]
    fn mouse_before_the_first_frame_does_nothing() {
        let mut app = sample();
        press(&mut app, KeyCode::Char('c'));
        assert_eq!(app.hits, None, "nothing drawn yet");
        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::ScrollDown,
            MouseEventKind::ScrollUp,
        ] {
            for (x, y) in [(0, 0), (5, 5), (30, 8), (20, 1)] {
                mouse(&mut app, kind, x, y);
                assert_eq!(app.status, Some("Link copied to clipboard!"));
                assert_eq!((app.mode, app.selected), (Mode::Feed, 0));
                assert_eq!((app.feed, app.sort), (Feed::Home, Sort::Hot));
            }
        }
    }

    #[test]
    fn stale_regions_never_break_invariants() {
        let mut app = thread_sample();
        let everywhere = Rect::new(0, 0, 100, 100);
        app.hits = Some(Hits {
            regions: vec![(everywhere, Target::Card(5))],
            max_scroll: 0,
        });
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 1, 1);
        assert_eq!((app.mode, app.selected), (Mode::Feed, 0), "no such card");

        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Enter);
        assert_eq!(visible_authors(&app), ["a", "f", "g"]);
        // b is hidden under the collapsed a, so it can't take the focus.
        app.hits = Some(Hits {
            regions: vec![(everywhere, Target::Comment(vec![0, 0]))],
            max_scroll: 0,
        });
        mouse(&mut app, MouseEventKind::Down(MouseButton::Left), 1, 1);
        assert_eq!(focused(&app), Some("a"));
        press_str(&mut app, "np");
        assert_eq!(focused(&app), Some("a"));
    }
}
