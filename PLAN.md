# workit — a Reddit-style TUI for Workplace posts

A terminal app that copies the layout of the reddit.com homepage, but every
post is a (fictional) Workplace post: groups instead of subreddits, coworkers
instead of redditors, reorgs instead of memes.

**Status:** implemented. This started as the pre-implementation plan and now
serves as design notes describing the code as built. When the two disagree,
fix whichever one is wrong. Don't let them drift apart.

## Goals

- At a glance it looks like the Reddit homepage: a header bar, a left nav, the
  center feed of post cards with vote arrows, and a right sidebar with
  community info.
- It responds the way Reddit does: voting, sort tabs (Hot / New / Top /
  Rising), filtering by group, joining and leaving groups, opening a post to
  read its comment tree, saving posts and listing them in a Saved feed.
- One self-contained binary. No network and no real Workplace data: all
  content is hard-coded, fictional, and safe for work.

## Non-goals

- Fetching real Workplace content (would need auth and APIs, and raises privacy
  concerns).
- Persisting state between runs. Votes, saves and group membership reset on
  restart.
- Mouse support.

## Build & run

A standard Cargo project.

```
cargo run
cargo test
```

- **Dependencies:** just `ratatui` (0.30). It re-exports `crossterm` as
  `ratatui::crossterm`, which keeps the backend and the event types on the same
  version.
- **Edition:** 2024, which the event loop needs for its let-chain.

## Layout

Captured from the real binary in a 110×24 tmux pane:

```
╭────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│ ◉  workit     ⌕ Search Home                                                           ● u/you  ✦ 1.3k karma│
╰────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭────────────────────╮╭ Home ────────────────────────────────────────────────╮╭ Home ────────────────────────╮
│ ⌂ Home             ││ 1 Hot   2 New   3 Top   4 Rising                     ││ Your personal workit         │
│ ↗ Popular          │╰──────────────────────────────────────────── 19 posts ╯│ frontpage. Come here to      │
│ ◎ All              │╭──────────────────────────────────────────────────────╮│ check in with your favorite  │
│ ☆ Saved            ││   ▲   w/Engineering · u/dana_k · 3h ago   Win        ││ groups.                      │
│                    ││ 1.2k  We deleted 40k lines of dead code and the      ││                              │
│ GROUPS             ││   ▼   build got 30% faster                           ││          Create Post         │
│ ● w/Engineering    ││       After six months of slowly untangling flags    │╰──────────────────────────────╯
│ ● w/RemoteWork     ││       that had been fully rolled out since 2021, th… │╭ Trending today ──────────────╮
│ ● w/OfficeLife     ││       ▤ 194 Comments   ↗ Share   ☆ Save   ···        ││ Hackathon 2026 is happening… │
│ ● w/Hackathon      │╰──────────────────────────────────────────────────────╯│ w/Announcements · 545 comme… │
│ ○ w/Announcements  │╭──────────────────────────────────────────────────────╮│ Biscuit got his own visitor… │
│ ● w/Foodies        ││   ▲   w/OfficeLife · u/priya_r · 1h ago   PSA        ││ w/DogsOfTheOffice · 406 com… │
│ ○ w/DogsOfTheOffice││  856  PSA: the espresso machine on floor 3 is FIXED  ││ Unpopular opinion: most of … │
│ ● w/OncallSurvivors││   ▼   Facilities replaced the grinder this morning.  ││ w/Engineering · 393 comments │
│                    ││       It now makes coffee instead of angry grinding… ││ New building opens Monday: … │
│                    ││       ▤ 100 Comments   ↗ Share   ☆ Save   ···        ││ w/Announcements · 332 comme… │
│                    │╰──────────────────────────────────────────────────────╯╰──────────────────────────────╯
│                    │╭──────────────────────────────────────────────────────╮
╰────────────────────╯╰──────────────────────────────────────────────────────╯
 a/z vote  enter open  1-4 sort  [/] feed  / search  s save  q quit
```

**No emoji, on purpose.** Every glyph is a single-width character: `◉ ⌕ ✦ ⌂ ↗ ◎ ● ○ ▲ ▼ ▤ ☆ ★ ··· ✔`. Emoji such as 🔥 or 💬 render two
cells wide in most terminals, and widely in tmux, which breaks the alignment
of the bordered cards. Don't add them.

The screen is a header (3 rows), a body, and a footer (1 row).

- **Header:** the `◉ workit` logo on Reddit orange and a search box. The box
  shows a dimmed `Search <feed>` placeholder. While typing it shows the live
  query with a cursor on a highlighted background. Once a search is kept, it
  shows the query plus `(esc to clear)`. On the right are `u/you` and the karma.
- **Left nav** (`NAV_WIDTH = 22`): Home / Popular / All / Saved, then every
  group. `●` marks a joined group and `○` an unjoined one, and the mark flips
  as soon as you join or leave. The active feed is orange on a dark
  background.
- **Feed:** a box titled with the feed name holds the numbered sort tabs. The
  active tab is orange and underlined, and the box's bottom edge shows the
  post count. Post cards follow. The selected card has an orange border and a
  highlighted vote column.
- **Right sidebar** (`SIDEBAR_WIDTH = 32`):
  - In a group feed, the top box is "About w/<group>": description, member and
    online counts, and a Joined/Join button showing the current membership.
    There's no mouse support, so `J` is what toggles it.
  - In Home, Popular, All and Saved, it is titled with the feed name and
    shows a generic blurb with a "Create Post" button, which is decorative.
  - Below that are "Trending today" (the 4 posts with the most comments,
    across all posts regardless of feed) and "Popular groups" (top 5 by
    members).
  - **Short terminals:** boxes stack in that priority order. A lower box
    that doesn't fit whole is skipped rather than squeezed, so About always
    keeps its button. At 24 rows, Popular groups disappears first. Only
    About is drawn when there isn't room for it, and it is clipped at the
    bottom.
- **Footer:** key hints, which change with the mode. There are separate sets
  for the feed, the feed with a kept search, search mode, and the post view.
  The plain feed set adds `J join` on a group feed only. Every set must fit on
  one line at 80 columns on every feed, so the feed set leaves out `j/k` and `c` (which the
  post view set still shows) and lists only `1-4` for sorting, not `Tab`. A
  status toast (green, `✔ …`) replaces the hints until the next key press.
- **Responsive:** the left nav shows only when the body is at least 70 columns
  wide, and the right sidebar only at 100 or more.

### Post cards

Card height is 2 border rows, plus 1 meta row, the wrapped title, up to 2
preview rows, and 1 action row. Here is what goes on each row:

- **Vote column:** `▲` / score / `▼`. An upvoted post shows the arrow and score
  in orange, a downvoted one in periwinkle. Scores of 1,000 or more are
  compacted (`1.2k`), so a ±1 vote on a big post changes only the color, as on
  Reddit.
- **Meta row:** `w/<group> · Posted by u/<author> · <age>`, then a colored flair
  tag. Ages read `just now`, `Nh ago` (under 24h) or `Nd ago`. When the row is
  too wide, it shortens in this order:
  1. drop "Posted by" (`w/<group> · u/<author> · <age>`),
  2. drop the author,
  3. drop the flair,
  4. truncate with `…`.

  The post view's meta row follows the same rule.
- **Preview:** the body with newlines flattened, wrapped, and cut to two rows
  with `…`.
- **Action row:** `▤ N Comments   ↗ Share   ☆ Save   ···`. Save becomes an
  orange `★ Saved`.

**Scrolling:** `App.offset` is the index of the first card drawn, and the
renderer maintains it. It never exceeds `selected`, and it advances until the
selected card fits entirely. The last card on screen may be clipped at the
bottom edge.

### Empty feed

A feed can end up with nothing to show: a search with no hits, Rising +
Popular, Rising on a group whose posts are all 24h or older, or Saved with
nothing saved. In that case the tab box stays, showing `0 posts`, and a
centered, wrapped message replaces the cards:

- With an active search: **"Hm... we couldn't find any results"** /
  "Double-check your spelling, or press esc to clear the search".
- In Saved, when no post is saved: **"No saved posts yet"** / "Press s on any
  post to save it".
- Otherwise, including Saved when Rising hides every saved post: **"Nothing
  here yet"** / "Try another sort with 1-4, or another feed with [ and ]".

In an empty feed, Enter, voting and saving do nothing.

### Post view (Enter)

The post replaces the feed column. The header, nav, sidebar and footer stay,
as on Reddit's post page. It shows:

- the meta row, the full title, the full body (newlines kept), and an action
  row with the votes,
- a rule and "Sort by: Best ▾" (decorative),
- the comment tree. Each reply level adds a `│ ` guide. Each comment shows its
  author and age, its wrapped body, `▲ score ▼   Reply   Share`, and a
  trailing guide line.
- "··· N more comments", where N is `comment_count` minus the comments in the
  seeded tree.

The renderer clamps the scroll position to the content height and writes it
back into `Mode::Post { scroll }`. So `G` followed by `k` moves right away,
instead of first counting down from `u16::MAX`.

## Data model (`src/data.rs`)

```rust
struct Group   { name, description, members, online, joined: bool }
struct Comment { author, body, score, hours_ago, replies: Vec<Comment> }
struct Post    { group: usize, author, title, body, flair: Option<&str>,
                 score, hours_ago, comment_count: u32,
                 comments: Vec<Comment>, vote: Vote, saved: bool }
enum Vote { Up, None /* default */, Down }
```

- **`comment_count` is the displayed total and `comments` is only a sample.**
  A card may say "194 Comments" while the tree holds 5, just as most of a
  Reddit thread sits behind "load more". Don't "fix" the count to
  `comments.len()` or `Comment::total()`. The post view shows the difference
  as "N more comments".
- **Posts have no ID.** They're identified by their index in `App.posts`,
  which never changes after startup.
- **Ages:** stored as `hours_ago`, not timestamps, so ordering and the tests
  are deterministic.
- **Groups:** 8 groups. Six start joined: Engineering, RemoteWork, OfficeLife,
  Hackathon, Foodies and OncallSurvivors. Announcements and DogsOfTheOffice
  start unjoined, so their posts appear in Popular, All and their own group
  feed, but not in Home. `J` on a group's feed joins or leaves it.
- **Posts:** 24, built as `Post { …, ..Default::default() }` literals. Every
  name in the seed data is invented.

## App state (`src/app.rs`)

| Field | Meaning |
|---|---|
| `groups`, `posts` | seed data. `posts` is mutated in place by votes and saves, `groups[g].joined` by `J` |
| `view: Vec<usize>` | indices into `posts` after feed filter, search and sort |
| `selected` | index into `view` |
| `offset` | first card drawn. Maintained by the renderer (see Scrolling) |
| `sort: Sort` | `Hot`, `New`, `Top`, `Rising` |
| `feed: Feed` | `Home`, `Popular`, `All`, `Saved`, `Group(usize)` |
| `mode: Mode` | `Feed`, `Search`, `Post { scroll: u16 }` |
| `query: String` | search text. Kept after leaving search mode with Enter |
| `status: Option<&'static str>` | toast text. Every toast is a fixed string. Cleared on the next key press |
| `karma: u32` | fixed at 1,337 (shown as `1.3k`). Votes don't change it |
| `should_quit` | checked by the event loop in `main.rs` |

### Feeds (which posts are shown)

- **Home:** posts from the groups that are joined when it is rebuilt. `J`
  only works on a group feed, so Home is never on screen while membership
  changes. Going back to it rebuilds it with the new membership.
- **Popular:** posts with score ≥ 500, under whatever sort is active. It is a
  filter only and doesn't force Hot.
- **All:** every post.
- **Saved:** posts with `saved` set, from any group, joined or not.
- **Group(g):** posts in group `g`, whether or not you've joined it.

`[` and `]` cycle Home → Popular → All → Saved → each group in seed order,
wrapping in both directions.

### Sorts (what order they're in)

- **Hot:** Reddit's formula, `signum(s)·log10(max(|s|, 1)) − hours_ago/12.5`.
  Reddit's 45000 s decay is 12.5 h. A score of 0 gives `signum` = 1.0 in Rust,
  but `log10(1)` = 0, so the term is still 0.
- **New:** `hours_ago` ascending.
- **Top:** score descending.
- **Rising:** drops posts 24h or older, then orders by
  `score / (hours_ago + 2)`. It filters as well as sorts, which is why it can
  empty a feed.

All of these use Rust's stable sorts, so **tied posts keep their seed order**.
The unit tests rely on that for deterministic results.

### Search

Search is a case-insensitive substring match over the title, body, author and
group name. It combines with the current feed and sort, and the view updates
on every keystroke.

### Selection behavior

- **Rebuilding resets to the top.** Every rebuild of `view` sets `selected`
  and `offset` back to 0. A rebuild happens on a feed change, a sort change,
  each search keystroke, and clearing the search.
- **Votes and saves don't rebuild.** Voting changes the score by ±1, or ±2
  when flipping an existing vote, and pressing the same vote again clears it.
  `view` is not re-sorted afterwards, so the selected card doesn't jump away
  under the cursor. Reddit also leaves the order alone until you refresh.
  Unsaving in the Saved feed works the same way, whether from the card or
  from the post view: the card stays, now showing `☆ Save`, and leaves the
  feed at the next rebuild.
- **Joining and leaving don't rebuild either.** A group feed's posts don't
  depend on membership, so `J` keeps `view` and `selected` as they are, and
  a kept search stays too.
- **These rules differ on purpose.** You change feed, sort or search to
  get a new list, so landing at its top is expected. You vote on a card while
  looking at it, so it should stay put.

## Key bindings

**Global:** Ctrl-C quits from any mode, including search.

| Key | Feed | Post view |
|---|---|---|
| `j`/`↓`, `k`/`↑` | select next / prev | scroll 1 line |
| `PgDn` / `PgUp` | move 5 cards | scroll 10 lines |
| `Space` | — | scroll 10 lines |
| `g`/`Home`, `G`/`End` | first / last card | top / bottom |
| `a` / `z` | upvote / downvote | same, for the post |
| `s` | save / unsave (toast) | same |
| `J` | join / leave the group (toast). Only on a group feed, with or without a kept search | — |
| `c` | "Link copied to clipboard!" toast | same |
| `Enter` | open post (does nothing in an empty feed) | — |
| `1`-`4` / `Tab` | set / cycle sort | — |
| `[` / `]` | prev / next feed | — |
| `/` | enter search mode, editing any kept query | — |
| `Esc` | clear a kept search. Otherwise nothing, so it never quits | back to feed |
| `q` | quit | back to feed |
| `Backspace` | — | back to feed |

`—` means the key does nothing. In particular, `J` does nothing in the post
view, even on a post from a group feed. Like sorting and switching feeds, it
acts on the feed, so you go back to the feed to join or leave.

**Search mode:**

- Every printable key, including `q`, `j`, `J` and digits, is typed into the
  query.
- Backspace deletes a character.
- Enter returns to the feed and keeps the filter.
- Esc returns to the feed and clears it.

## Files

```
workit/
  PLAN.md
  Cargo.toml    edition 2024, depends on ratatui
  src/
    main.rs     ratatui::init()/restore() (restores the terminal on panic), event loop
    app.rs      state, feeds, sorts, voting, saving, joining, search, key handling. No rendering
    data.rs     seed groups, posts, comments
    ui.rs       all rendering, including offset upkeep and post-view scroll clamping
```

## Testing

31 unit tests, run with `cargo test`:

- **`app.rs`** (19), on a small custom data set:
  - Home hides unjoined groups, and Popular hides low scores.
  - The exact order for each sort.
  - Feed cycling wraps in both directions, with Saved between All and the
    groups.
  - Joining a group adds its posts to Home and leaving one removes them.
    `J` on a group feed keeps `view` and `selected`, and a kept search. It
    does nothing on Home, Popular, All and Saved or in the post view, and is
    typed into the query in search mode.
  - Saved lists exactly the saved posts, combined with a search and with
    Rising. Unsaving there, from the card or from the post view, keeps the
    card until the next rebuild.
  - Vote toggling and flipping, without reordering the feed.
  - Selection clamping.
  - Search: live filtering, typing `q` doesn't quit, Esc clears.
  - Esc in the feed never quits.
  - Opening and leaving the post view, and Enter in an empty feed.
- **`ui.rs`** (12), rendering into a `TestBackend`:
  - The helpers `compact`, `age`, `wrap` and `truncate`.
  - The frontpage contents.
  - Sidebars hidden at narrow widths.
  - Sidebar boxes skipped, not squeezed, at 24 rows.
  - The meta row's shortening order, and that it never overflows at any
    width.
  - The selected card staying visible after `G`.
  - Both empty-feed messages, and the empty-Saved one (but not when a search
    or Rising is what empties Saved).
  - Comments in the post view, and scroll clamping.
  - `☆ Saved` right below `◎ All` in the nav, and Saved's sidebar.
  - The group sidebar button and nav mark flipping with `J`.
  - Every hint set (feed, kept search, search, post view) fitting in 80
    columns on every feed, with `J join` only in the feed set on group feeds.

Lint with `arc f`, `arc lint` and `arc rust-clippy`. The `arc lint -e extra`
engine isn't configured for `users/`.

## Ideas

Voting on comments, collapsible threads, mouse support.
