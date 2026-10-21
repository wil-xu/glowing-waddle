use std::cmp::Reverse;
use std::collections::HashSet;

use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::crossterm::event::KeyModifiers;

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

/// Comment focus and collapse state for the open post, by `Drawn::id`. Reset
/// when a post is opened and when it is left, so outside the post view it is
/// always the default.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Thread {
    pub focus: Option<usize>,
    pub collapsed: HashSet<usize>,
    /// Set when the focus moves or the focused comment is voted on or
    /// collapsed. The renderer scrolls its header into view once and clears
    /// it, so scrolling afterwards isn't pulled back.
    pub reveal: bool,
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
    pub thread: Thread,
    pub query: String,
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

/// Casts `vote` on a post or comment: ±1, ±2 when flipping an existing vote,
/// and repeating a vote clears it.
fn cast(score: &mut i32, current: &mut Vote, vote: Vote) {
    let value = |v: Vote| match v {
        Vote::Up => 1,
        Vote::None => 0,
        Vote::Down => -1,
    };
    let new = if *current == vote { Vote::None } else { vote };
    *score += value(new) - value(*current);
    *current = new;
}

/// A comment the post view draws.
pub(crate) struct Drawn<'a> {
    /// Pre-order index in the post's whole seeded tree, hidden comments
    /// included. The tree never changes shape, so neither does the id.
    pub id: usize,
    pub comment: &'a Comment,
    /// Indices from the top level down to this comment.
    path: Vec<usize>,
}

impl Drawn<'_> {
    /// 0 for a top-level comment, 1 for a reply, and so on.
    pub(crate) fn depth(&self) -> usize {
        self.path.len() - 1
    }
}

/// The comments the post view draws, in display order: a comment, then its
/// replies, then its next sibling. A collapsed comment is drawn but its
/// descendants are skipped. This is the only place comment ids are assigned.
pub(crate) fn drawn_comments<'a>(
    comments: &'a [Comment],
    collapsed: &HashSet<usize>,
) -> Vec<Drawn<'a>> {
    fn walk<'a>(
        comments: &'a [Comment],
        collapsed: &HashSet<usize>,
        next_id: &mut usize,
        path: &mut Vec<usize>,
        out: &mut Vec<Drawn<'a>>,
    ) {
        for (i, comment) in comments.iter().enumerate() {
            let id = *next_id;
            *next_id += 1;
            path.push(i);
            out.push(Drawn {
                id,
                comment,
                path: path.clone(),
            });
            if collapsed.contains(&id) {
                // The hidden descendants keep their ids.
                *next_id += comment.total() as usize - 1;
            } else {
                walk(&comment.replies, collapsed, next_id, path, out);
            }
            path.pop();
        }
    }
    let mut out = Vec::new();
    walk(comments, collapsed, &mut 0, &mut Vec::new(), &mut out);
    out
}

impl App {
    pub(crate) fn new() -> Self {
        Self::with_data(data::groups(), data::posts())
    }

    fn with_data(groups: Vec<Group>, posts: Vec<Post>) -> Self {
        let mut app = App {
            groups,
            posts,
            view: Vec::new(),
            selected: 0,
            offset: 0,
            sort: Sort::Hot,
            feed: Feed::Home,
            mode: Mode::Feed,
            thread: Thread::default(),
            query: String::new(),
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

    fn matches_query(&self, post: &Post) -> bool {
        if self.query.is_empty() {
            return true;
        }
        let q = self.query.to_lowercase();
        [
            post.title,
            post.body,
            post.author,
            self.groups[post.group].name,
        ]
        .iter()
        .any(|field| field.to_lowercase().contains(&q))
    }

    /// Rebuilds `view` and resets the selection to the top of the feed.
    fn refresh(&mut self) {
        let mut view: Vec<usize> = (0..self.posts.len())
            .filter(|&i| {
                let post = &self.posts[i];
                self.in_feed(post)
                    && self.matches_query(post)
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
        cast(&mut post.score, &mut post.vote, vote);
    }

    /// In the post view, votes on the focused comment, or on the post when no
    /// comment is focused. Comments are never re-sorted, and the focused one
    /// is scrolled back into view so you see what you voted on.
    fn vote_in_post(&mut self, vote: Vote) {
        let Some(id) = self.thread.focus else {
            self.vote(vote);
            return;
        };
        if let Some(comment) = self.drawn_comment_mut(id) {
            cast(&mut comment.score, &mut comment.vote, vote);
            self.thread.reveal = true;
        }
    }

    /// The open post's comment with id `id`, if it is drawn.
    fn drawn_comment_mut(&mut self, id: usize) -> Option<&mut Comment> {
        let &i = self.view.get(self.selected)?;
        let comments = &mut self.posts[i].comments;
        let path = drawn_comments(comments, &self.thread.collapsed)
            .into_iter()
            .find(|d| d.id == id)?
            .path;
        let (&top, rest) = path.split_first()?;
        Some(
            rest.iter()
                .fold(&mut comments[top], |c, &j| &mut c.replies[j]),
        )
    }

    /// Focuses the next or previous drawn comment, stopping at both ends.
    /// With no focus, only `forward` does anything: it focuses the first one.
    fn move_focus(&mut self, forward: bool) {
        let Some(post) = self.selected_post() else {
            return;
        };
        let drawn = drawn_comments(&post.comments, &self.thread.collapsed);
        let pos = self
            .thread
            .focus
            .and_then(|f| drawn.iter().position(|d| d.id == f));
        let next = match pos {
            None if forward => drawn.first(),
            None => None,
            Some(pos) if forward => drawn.get(pos + 1),
            Some(pos) => pos.checked_sub(1).and_then(|p| drawn.get(p)),
        };
        if let Some(&Drawn { id, .. }) = next {
            self.thread.focus = Some(id);
            self.thread.reveal = true;
        }
    }

    fn toggle_collapse(&mut self) {
        let Some(id) = self.thread.focus else {
            return;
        };
        if !self.thread.collapsed.remove(&id) {
            self.thread.collapsed.insert(id);
        }
        self.thread.reveal = true;
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
            KeyCode::Enter if !self.view.is_empty() => {
                self.mode = Mode::Post { scroll: 0 };
                self.thread = Thread::default();
            }
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
    /// the end here is harmless.
    fn on_post_key(&mut self, code: KeyCode, scroll: u16) {
        let scroll = match code {
            KeyCode::Esc if self.thread.focus.is_some() => {
                self.thread.focus = None;
                scroll
            }
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Backspace => {
                self.mode = Mode::Feed;
                self.thread = Thread::default();
                return;
            }
            KeyCode::Char('j') | KeyCode::Down => scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => scroll.saturating_sub(1),
            KeyCode::PageDown | KeyCode::Char(' ') => scroll.saturating_add(10),
            KeyCode::PageUp => scroll.saturating_sub(10),
            KeyCode::Char('g') | KeyCode::Home => 0,
            KeyCode::Char('G') | KeyCode::End => u16::MAX,
            KeyCode::Char('n') => {
                self.move_focus(true);
                scroll
            }
            KeyCode::Char('p') => {
                self.move_focus(false);
                scroll
            }
            KeyCode::Enter => {
                self.toggle_collapse();
                scroll
            }
            KeyCode::Char('a') => {
                self.vote_in_post(Vote::Up);
                scroll
            }
            KeyCode::Char('z') => {
                self.vote_in_post(Vote::Down);
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

    fn comment(author: &'static str, replies: Vec<Comment>) -> Comment {
        Comment {
            author,
            body: "",
            score: 10,
            hours_ago: 1,
            replies,
            vote: Vote::None,
        }
    }

    /// `sample()` with a comment tree on "mid", the first post on Home:
    /// a > b > c > d, a > e, then f.
    fn threaded() -> App {
        let mut app = sample();
        app.posts[3].comments = vec![
            comment(
                "a",
                vec![
                    comment("b", vec![comment("c", vec![comment("d", vec![])])]),
                    comment("e", vec![]),
                ],
            ),
            comment("f", vec![]),
        ];
        assert_eq!(app.view[0], 3);
        app
    }

    fn authors(comments: &[Comment]) -> Vec<&'static str> {
        comments
            .iter()
            .flat_map(|c| std::iter::once(c.author).chain(authors(&c.replies)))
            .collect()
    }

    fn focused(app: &App) -> Option<&'static str> {
        let post = app.selected_post().expect("a post is open");
        app.thread.focus.map(|id| authors(&post.comments)[id])
    }

    /// Presses `code` and returns the focused comment's author.
    fn focus_after(app: &mut App, code: KeyCode) -> Option<&'static str> {
        press(app, code);
        focused(app)
    }

    #[test]
    fn focus_keys_do_nothing_on_a_post_without_comments() {
        let mut app = sample();
        assert!(app.posts[3].comments.is_empty());
        press(&mut app, KeyCode::Enter);
        for code in [
            KeyCode::Char('n'),
            KeyCode::Char('p'),
            KeyCode::Enter,
            KeyCode::Char('n'),
        ] {
            press(&mut app, code);
            assert_eq!(app.thread.focus, None, "{code:?}");
            assert!(app.thread.collapsed.is_empty(), "{code:?}");
            assert_eq!(app.mode, Mode::Post { scroll: 0 }, "{code:?}");
        }
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(app.posts[3].vote, Vote::Up, "a still votes on the post");
    }

    #[test]
    fn n_and_p_walk_comments_in_pre_order_and_stop_at_both_ends() {
        let mut app = threaded();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.thread.focus, None, "nothing is focused on open");
        assert_eq!(focus_after(&mut app, KeyCode::Char('p')), None);

        let forward: Vec<_> = (0..7)
            .map(|_| focus_after(&mut app, KeyCode::Char('n')))
            .collect();
        assert_eq!(
            forward,
            ["a", "b", "c", "d", "e", "f", "f"].map(Some),
            "a comment, then its replies, then its next sibling"
        );
        let back: Vec<_> = (0..6)
            .map(|_| focus_after(&mut app, KeyCode::Char('p')))
            .collect();
        assert_eq!(back, ["e", "d", "c", "b", "a", "a"].map(Some));
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
    }

    #[test]
    fn n_and_p_skip_collapsed_replies_and_each_comment_keeps_its_state() {
        let mut app = threaded();
        press(&mut app, KeyCode::Enter);
        press_str(&mut app, "nnn");
        assert_eq!(focused(&app), Some("c"));
        assert_eq!(focus_after(&mut app, KeyCode::Enter), Some("c"));
        assert!(app.thread.collapsed.contains(&2));
        assert_eq!(focus_after(&mut app, KeyCode::Char('n')), Some("e"));
        assert_eq!(focus_after(&mut app, KeyCode::Char('p')), Some("c"));

        assert_eq!(focus_after(&mut app, KeyCode::Char('p')), Some("b"));
        assert_eq!(
            focus_after(&mut app, KeyCode::Enter),
            Some("b"),
            "focus stays on a comment when it collapses"
        );
        assert_eq!(focus_after(&mut app, KeyCode::Char('n')), Some("e"));
        assert_eq!(focus_after(&mut app, KeyCode::Char('p')), Some("b"));

        press(&mut app, KeyCode::Enter);
        assert!(!app.thread.collapsed.contains(&1), "b is expanded again");
        assert!(app.thread.collapsed.contains(&2), "c is still collapsed");
        assert_eq!(focus_after(&mut app, KeyCode::Char('n')), Some("c"));
        assert_eq!(focus_after(&mut app, KeyCode::Char('n')), Some("e"));
    }

    #[test]
    fn comment_votes_toggle_and_flip_without_touching_the_post() {
        let mut app = threaded();
        app.posts[3].comments[1].score = 1;
        let order = authors(&app.posts[3].comments);
        press(&mut app, KeyCode::Enter);
        press_str(&mut app, "nn");
        assert_eq!(focused(&app), Some("b"));
        let b = |app: &App| {
            let c = &app.posts[3].comments[0].replies[0];
            (c.score, c.vote)
        };

        press(&mut app, KeyCode::Char('a'));
        assert_eq!(b(&app), (11, Vote::Up));
        press(&mut app, KeyCode::Char('z'));
        assert_eq!(b(&app), (9, Vote::Down), "flipping up to down is -2");
        press(&mut app, KeyCode::Char('z'));
        assert_eq!(b(&app), (10, Vote::None), "repeating a vote clears it");
        press(&mut app, KeyCode::Char('z'));
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(b(&app), (11, Vote::Up), "flipping down to up is +2");

        // Upvote f, the lowest scored comment, well past the others.
        press_str(&mut app, "nnnn");
        assert_eq!(focused(&app), Some("f"));
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(app.posts[3].comments[1].score, 2);

        assert_eq!(app.posts[3].score, 600, "the post's score is untouched");
        assert_eq!(app.posts[3].vote, Vote::None);
        assert_eq!(authors(&app.posts[3].comments), order, "never reordered");
        assert_eq!(focused(&app), Some("f"));
    }

    #[test]
    fn comment_votes_survive_reopening_but_focus_and_collapse_reset() {
        let mut app = threaded();
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Enter);
        assert!(app.thread.collapsed.contains(&0));
        press(&mut app, KeyCode::Char('q'));
        assert_eq!(app.mode, Mode::Feed);
        assert_eq!(app.thread, Thread::default(), "reset on leaving");

        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, Mode::Post { scroll: 0 });
        assert_eq!(app.thread.focus, None, "focus resets on open");
        assert!(app.thread.collapsed.is_empty(), "collapse resets on open");
        let a = &app.posts[3].comments[0];
        assert_eq!((a.score, a.vote), (11, Vote::Up), "the vote is kept");
        assert_eq!(
            focus_after(&mut app, KeyCode::Char('n')),
            Some("a"),
            "n starts from the top again"
        );
        assert_eq!(focus_after(&mut app, KeyCode::Char('n')), Some("b"));
    }

    #[test]
    fn esc_unfocuses_before_leaving_and_q_and_backspace_leave_directly() {
        let mut app = threaded();
        press(&mut app, KeyCode::Enter);
        press_str(&mut app, "nj");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.thread.focus, None);
        assert_eq!(app.mode, Mode::Post { scroll: 1 }, "still in the post");
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.mode, Mode::Feed);

        assert_eq!(app.thread, Thread::default());

        for code in [KeyCode::Char('q'), KeyCode::Backspace] {
            press(&mut app, KeyCode::Enter);
            press(&mut app, KeyCode::Char('n'));
            press(&mut app, KeyCode::Enter);
            assert!(app.thread.reveal && app.thread.collapsed.contains(&0));
            press(&mut app, code);
            assert_eq!(app.mode, Mode::Feed, "{code:?} leaves while focused");
            assert!(!app.should_quit);
            assert_eq!(
                app.thread,
                Thread::default(),
                "{code:?} leaves no state for a post that isn't open"
            );
        }
    }

    #[test]
    fn save_and_share_act_on_the_post_while_focused_and_enter_needs_focus() {
        let mut app = threaded();
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        assert!(app.thread.collapsed.is_empty(), "nothing to collapse");
        assert_eq!(app.mode, Mode::Post { scroll: 0 });

        press(&mut app, KeyCode::Char('n'));
        press(&mut app, KeyCode::Char('s'));
        assert!(app.posts[3].saved);
        assert_eq!(app.status, Some("Post saved"));
        press(&mut app, KeyCode::Char('c'));
        assert_eq!(app.status, Some("Link copied to clipboard!"));
        assert_eq!(focused(&app), Some("a"), "the focus is kept");
        press(&mut app, KeyCode::Char('J'));
        assert!(
            app.groups[0].joined,
            "J still does nothing in the post view"
        );
    }

    #[test]
    fn n_p_and_enter_keep_their_feed_and_search_meanings() {
        let mut app = threaded();
        press_str(&mut app, "jnp");
        assert_eq!(app.mode, Mode::Feed);
        assert_eq!(app.selected, 1, "n and p don't move the selection");
        assert_eq!(app.view.len(), 3);

        press(&mut app, KeyCode::Char('/'));
        press_str(&mut app, "np");
        assert_eq!(app.query, "np", "n and p are typed while searching");
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.mode, Mode::Post { scroll: 0 }, "Enter opens a post");
    }
}
