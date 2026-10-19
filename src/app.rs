use std::cmp::Reverse;

use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::event::KeyEvent;
use ratatui::crossterm::event::KeyModifiers;

use crate::data;
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
        let value = |v: Vote| match v {
            Vote::Up => 1,
            Vote::None => 0,
            Vote::Down => -1,
        };
        let new = if post.vote == vote { Vote::None } else { vote };
        post.score += value(new) - value(post.vote);
        post.vote = new;
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
            KeyCode::Enter if !self.view.is_empty() => self.mode = Mode::Post { scroll: 0 },
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
            KeyCode::Char('a') => {
                self.vote(Vote::Up);
                scroll
            }
            KeyCode::Char('z') => {
                self.vote(Vote::Down);
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
}
