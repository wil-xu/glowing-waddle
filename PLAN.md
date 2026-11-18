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
  read its comment tree (voting on comments and collapsing threads), saving
  posts and listing them in a Saved feed, and hiding posts, which then show up
  only in a Hidden feed where they can be unhidden.
- Like Reddit, the feed has a Card view (a bordered card per post, the
  default) and a Compact view (one row per post), switched at runtime with
  `v` or the switch on the sort-tab row.
- Votes, saves, hidden posts and group membership survive a restart (see
  Persistence).
- One self-contained binary. No network and no real Workplace data: all
  content is hard-coded, fictional, and safe for work.

## Non-goals

- Fetching real Workplace content (would need auth and APIs, and raises privacy
  concerns).
- Persisting anything beyond votes, saves, hidden posts and group membership.
  The feed, sort, search, Card/Compact view, selection, scroll, comment focus
  and collapse state and the toast start fresh on every run, on Home, Hot, in
  the Card view, at the top.

## Build & run

A standard Cargo project.

```
cargo run
cargo test
```

- **State file:** `WORKIT_STATE=/tmp/scratch cargo run` keeps votes, saves,
  hidden posts and membership somewhere other than the default (see
  Persistence).
- **Dependencies:** just `ratatui` (0.30). It re-exports `crossterm` as
  `ratatui::crossterm`, which keeps the backend and the event types on the same
  version.
- **Edition:** 2024, which the renderer needs for its let-chain.

## Layout

Captured from the real binary in a 110×24 tmux pane:

```
╭────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│ ◉  workit     ⌕ Search Home                                                           ● u/you  ✦ 1.3k karma│
╰────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭────────────────────╮╭ Home ────────────────────────────────────────────────╮╭ Home ────────────────────────╮
│ ⌂ Home             ││ 1 Hot   2 New   3 Top   4 Rising              ▦ Card ││ Your personal workit         │
│ ↗ Popular          │╰──────────────────────────────────────────── 19 posts ╯│ frontpage. Come here to      │
│ ◎ All              │╭──────────────────────────────────────────────────────╮│ check in with your favorite  │
│ ☆ Saved            ││   ▲   w/Engineering · u/dana_k · 3h ago   Win        ││ groups.                      │
│ ⊘ Hidden           ││ 1.2k  We deleted 40k lines of dead code and the      ││                              │
│                    ││   ▼   build got 30% faster                           ││          Create Post         │
│ GROUPS             ││       After six months of slowly untangling flags    │╰──────────────────────────────╯
│ ● w/Engineering    ││       that had been fully rolled out since 2021, th… │╭ Trending today ──────────────╮
│ ● w/RemoteWork     ││       ▤ 194 Comments   ↗ Share   ☆ Save   ···        ││ Hackathon 2026 is happening… │
│ ● w/OfficeLife     │╰──────────────────────────────────────────────────────╯│ w/Announcements · 545 comme… │
│ ● w/Hackathon      │╭──────────────────────────────────────────────────────╮│ Biscuit got his own visitor… │
│ ○ w/Announcements  ││   ▲   w/OfficeLife · u/priya_r · 1h ago   PSA        ││ w/DogsOfTheOffice · 406 com… │
│ ● w/Foodies        ││  856  PSA: the espresso machine on floor 3 is FIXED  ││ Unpopular opinion: most of … │
│ ○ w/DogsOfTheOffice││   ▼   Facilities replaced the grinder this morning.  ││ w/Engineering · 393 comments │
│ ● w/OncallSurvivors││       It now makes coffee instead of angry grinding… ││ New building opens Monday: … │
│                    ││       ▤ 100 Comments   ↗ Share   ☆ Save   ···        ││ w/Announcements · 332 comme… │
│                    │╰──────────────────────────────────────────────────────╯╰──────────────────────────────╯
│                    │╭──────────────────────────────────────────────────────╮
╰────────────────────╯╰──────────────────────────────────────────────────────╯
 a/z vote  1-4 sort  [/] feed  / search  s save  h hide  v view  q quit
```

The same screen in the Compact view (`v`, then `j` to select the second post
and clear the toast). The selected row is drawn on the highlight background
with an orange title, which a capture doesn't show:

```
╭────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│ ◉  workit     ⌕ Search Home                                                           ● u/you  ✦ 1.3k karma│
╰────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭────────────────────╮╭ Home ────────────────────────────────────────────────╮╭ Home ────────────────────────╮
│ ⌂ Home             ││ 1 Hot   2 New   3 Top   4 Rising           ≡ Compact ││ Your personal workit         │
│ ↗ Popular          │╰──────────────────────────────────────────── 19 posts ╯│ frontpage. Come here to      │
│ ◎ All              │ ▲  1.2k ▼ ☆ We deleted 40k lines of d…  3h ago · ▤ 194 │ check in with your favorite  │
│ ☆ Saved            │ ▲   856 ▼ ☆ PSA: the espresso machine…  1h ago · ▤ 100 │ groups.                      │
│ ⊘ Hidden           │ ▲  1.9k ▼ ☆ Got paged at 3:12am becau…  9h ago · ▤ 305 │                              │
│                    │ ▲   512 ▼ ☆ Ranking every cafe pizza …  4h ago · ▤ 154 │          Create Post         │
│ GROUPS             │ ▲    1k ▼ ☆ The boba cart is back on T…  8h ago · ▤ 91 ╰──────────────────────────────╯
│ ● w/Engineering    │ ▲   634 ▼ ☆ What's your best trick fo…  7h ago · ▤ 215 ╭ Trending today ──────────────╮
│ ● w/RemoteWork     │ ▲   388 ▼ ☆ First oncall shift starts…  6h ago · ▤ 113 │ Hackathon 2026 is happening… │
│ ● w/OfficeLife     │ ▲   201 ▼ ☆ It is 4am. Our demo is in …  3h ago · ▤ 31 │ w/Announcements · 545 comme… │
│ ● w/Hackathon      │ ▲   156 ▼ ☆ Is it normal to feel lost …  2h ago · ▤ 73 │ Biscuit got his own visitor… │
│ ○ w/Announcements  │ ▲    92 ▼ ☆ To everyone scheduling 8am…  1h ago · ▤ 19 │ w/DogsOfTheOffice · 406 com… │
│ ● w/Foodies        │ ▲   921 ▼ ☆ Unpopular opinion: most …  14h ago · ▤ 393 │ Unpopular opinion: most of … │
│ ○ w/DogsOfTheOffice│ ▲   298 ▼ ☆ LF 2 people for a hackath…  11h ago · ▤ 42 │ w/Engineering · 393 comments │
│ ● w/OncallSurvivors│ ▲   267 ▼ ☆ The salad bar added a 'bu…  16h ago · ▤ 61 │ New building opens Monday: … │
│                    │ ▲   433 ▼ ☆ Who keeps setting the 4t…  20h ago · ▤ 122 │ w/Announcements · 332 comme… │
│                    │ ▲   512 ▼ ☆ This test has failed 1 i…  22h ago · ▤ 101 ╰──────────────────────────────╯
│                    │ ▲  1.4k ▼ ☆ My cat joined a 200-perso…  1d ago · ▤ 262
╰────────────────────╯ ▲   744 ▼ ☆ Our team built a bot that…  1d ago · ▤ 151
 a/z vote  1-4 sort  [/] feed  / search  s save  h hide  v view  q quit
```

**No emoji, on purpose.** Every glyph is a single-width character:
`◉ ⌕ ✦ ⌂ ↗ ◎ ● ○ ▲ ▼ ▤ ☆ ★ ⊘ ··· ✔ ▦ ≡`. Emoji such as 🔥 or 💬 render two cells
wide in most terminals, and widely in tmux, which breaks the alignment of the
bordered cards. Don't add them. Check new symbols too: `☰` (the trigram,
an obvious "list" icon) was tried for the Compact switch and dropped. It is
Wide since Unicode 16, so ratatui (`unicode-width` 0.2) gives it two cells
while tmux 3.7 draws it in one, and the label came out with a gap. A test
checks that `▦` and `≡` are one cell each in ratatui and appear nowhere else
on screen.

The screen is a header (3 rows), a body, and a footer (1 row).

- **Header:** the `◉ workit` logo on Reddit orange and a search box. The box
  shows a dimmed `Search <feed>` placeholder. While typing it shows the live
  query, raw as typed (quotes and filters included), with a cursor on a
  highlighted background. Once a search is kept, it shows the query plus
  `(esc to clear)`. On the right are `u/you` and the karma.
- **Left nav** (`NAV_WIDTH = 22`): Home / Popular / All / Saved / Hidden
  (`⊘ Hidden`, right below `☆ Saved`), then every group. `●` marks a joined
  group and `○` an unjoined one, and the mark flips as soon as you join or
  leave. The active feed is orange on a dark background.
- **Feed:** a box titled with the feed name holds the numbered sort tabs. The
  active tab is orange and underlined, and the box's bottom edge shows the
  post count. Right-aligned on the tab row, with one blank column before the
  border, is the view switch: `▦ Card` or `≡ Compact` (an orange glyph and
  the current view's name in white). It is drawn only when there is at least
  one blank column between it and the end of `Rising`, so it never overlaps a
  tab: the feed column needs at least 46 columns for `≡ Compact` and 43 for
  `▦ Card`. Clicking it toggles the view like `v`. Post cards, or compact
  rows, follow. The selected card has an orange border and a highlighted vote
  column.
- **Right sidebar** (`SIDEBAR_WIDTH = 32`):
  - In a group feed, the top box is "About w/<group>": description, member and
    online counts, and a Joined/Join button showing the current membership.
    The button isn't clickable: `J` is what toggles it.
  - In Home, Popular, All, Saved and Hidden, it is titled with the feed name
    and shows a generic blurb with a decorative "Create Post" button.
  - Below that are "Trending today" (the 4 posts with the most comments,
    across all posts regardless of feed, except hidden ones: hiding a trending
    post lets the 5th take its place) and "Popular groups" (top 5 by members).
  - **Short terminals:** boxes stack in that priority order. A lower box
    that doesn't fit whole is skipped rather than squeezed, so About always
    keeps its button. At 24 rows, Popular groups disappears first. Only
    About is drawn when there isn't room for it, and it is clipped at the
    bottom.
- **Footer:** key hints, which change with the mode. There are separate sets
  for the feed, the feed with a kept search, search mode, and the post view
  with and without a focused comment. The plain feed set is `a/z vote`,
  `1-4 sort`, `[/] feed`, `/ search`, `s save`, `h hide`, `v view`, `q quit`
  (72 columns). On a group feed it adds `J join` before `v view` (80
  columns, exactly the limit). Neither shows `enter open`. It was first
  dropped from the group set alone, when `J join` would have made that set 84
  columns: Enter is the most guessable key, and a click on a card opens it
  too. Adding `v view` would have made the plain set 84, so it now leaves
  out `enter open` as well, for the same reason (a click on a compact row
  opens it too), while the group set comes to exactly 80. Dropping it was
  the cheapest fix: every other hint is
  a key you can't guess, and shortening labels (`[] feed`, `/ find`) saved
  only a column or two each. It also keeps the two sets the same apart from
  `J join`. The feed set with a kept search is unchanged and still lists
  `enter open`. The search set is
  `type filter`, `w/ u/ flair: narrow`, `enter keep`, `esc clear`. The post
  view set is `j/k scroll`, `n/p comments`, `a/z vote`, `s save`, `c share`,
  `esc back`, and with a comment focused `j/k scroll`, `n/p comments`,
  `a/z vote comment`, `enter collapse`, `esc unfocus`. Every set must fit on
  one line at 80 columns on every feed, Hidden included, so the feed set
  leaves out `j/k` and `c` (which the post view set still shows) and lists
  only `1-4` for sorting, not `Tab`. The hints are the same in both views. A
  status toast (green, `✔ …`) replaces
  the hints until the next key press, left click or wheel event (see Mouse).
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
- **Search highlighting:** every occurrence of a free-text search term (see
  Search) in the title and the two preview rows is drawn black on orange. The
  title keeps its bold, and the original-case text is shown. Overlapping
  matches merge into one run, a match that wraps highlights its part on each
  row (the line break draws nothing), and when the preview is cut only the
  visible part of a match is highlighted, never the `…`. The meta and action
  rows aren't highlighted. Highlighting only restyles cells: wrapping,
  `card_height` and scrolling are the same as without it, and with no
  free-text terms a card renders exactly as it would with no query.

### Compact rows

In the Compact view every post is exactly one row, with no border, in the
feed area below the tab box. The rows are `compact_row` in `ui.rs`, from left
to right:

```
 ▲  1.2k ▼ ★ We deleted 40k lines of dead code and t…  w/Engineering · 3h ago · ▤ 194
```

- a blank column, then the vote cluster `▲ <score> ▼`. The score is
  `compact()` right-aligned in `SCORE_WIDTH` = 5 columns (enough for
  `-1.2k` or `99.9k`, and a test checks every seed score ±1 fits), so the
  arrows, the star and the title start in the same columns on every row.
  Colors are the cards': an upvote draws `▲` and the score orange, a
  downvote `▼` and the score periwinkle, and otherwise the arrows are dim
  and the score white. All bold.
- `★` (orange) for a saved post, `☆` (dim) otherwise, then a blank. The
  title starts in column 13.
- the title, bold, whitespace runs shown as one space (as on cards), cut
  with `…` to the room left.
- right-aligned and dim, `w/<group> · <age> · ▤ <comments>`, with `age` and
  `compact` as on cards, at least 2 blank columns after the title and 1
  blank column before the right edge.

**Shortening.** The meta shortens so that the title keeps at least
`MIN_TITLE` = 20 columns of room when it can. It drops, in order, the group
(`<age> · ▤ <comments>`), then the comment count (`<age>`), then the whole
meta. Only after that does the title's room go below 20. The room counts, not
the title's length, so at a given width every row shortens the same way
(apart from group names and counts of different lengths). With the 56-column
feed of a 110- or 80-column terminal that means `<age> · ▤ <comments>`. Below
14 columns there is no room for a title, and the row is just its first
`width` cells: it never draws past its area, down to 1 column.

**Selected row.** The whole row is on `HIGHLIGHT_BG` and its title is orange
(bold), like the active nav entry and the focused comment header. The vote
and star colors don't change.

**Search highlighting** works as on cards: every occurrence of a free-text
term in the visible part of the title is black on orange, in the original
case and still bold. A match cut by the `…` highlights only its visible part,
and the `…` keeps the title style. The meta isn't highlighted. With no
free-text terms a row renders exactly as with no query. It uses the same
`highlight_rows`, with one row whose leading text chars stop before the `…`.

**Scrolling:** `App.offset` is the index of the first card (or compact row)
drawn, and the renderer maintains it. It never exceeds `selected`, and it
advances until the selected card fits entirely. The last card on screen may
be clipped at the bottom edge. The Compact view uses the same rule with every
post 1 row high, so 17 posts fit at 24 rows against 2½ cards. Toggling the
view leaves `offset` alone and the first frame applies the rule: from Compact
to Card with a post far down selected, `offset` advances until that card is
fully on screen. From Card to Compact nothing needs to scroll (the cards from
`offset` to `selected` fit, so their rows do), and the top post stays at the
top.

### Empty feed

A feed can end up with nothing to show: a search with no hits, Rising +
Popular, Rising on a group whose posts are all 24h or older, Saved with
nothing saved, Hidden with nothing hidden, or a feed whose last card was
hidden. In that case the tab box stays, showing `0 posts`, and a centered,
wrapped message replaces the cards:

- With an active search: **"Hm... we couldn't find any results"** /
  "Double-check your spelling, or press esc to clear the search". A query
  only counts when it has a free-text term or a filter. One with no effective
  terms (`w/`, `   `, `" "`) can't be what emptied the feed, so the message is
  chosen as if there were no query. The header still shows it.
- In Saved, when no post is saved: **"No saved posts yet"** / "Press s on any
  post to save it".
- In Hidden, when no post is hidden: **"No hidden posts"** / "Press h on any
  post to hide it".
- Otherwise, including Saved when Rising hides every saved post or every saved
  post is hidden, and Hidden when Rising hides every hidden post: **"Nothing
  here yet"** / "Try another sort with 1-4, or another feed with [ and ]".

In an empty feed, Enter, voting, saving and hiding do nothing.

### Post view (Enter)

The post replaces the feed column. The header, nav, sidebar and footer stay,
as on Reddit's post page. It shows:

- the meta row, the full title, the full body (newlines kept), and an action
  row with the votes,
- a rule and "Sort by: Best ▾" (decorative),
- the comment tree, in pre-order (a comment, then its replies, then its next
  sibling). Each reply level adds a `│ ` guide. Each comment shows its
  author and age, its wrapped body, `▲ score ▼   Reply   Share`, and a
  trailing guide line. `Reply` and `Share` are decorative.
- "··· N more comments", where N is `comment_count` minus the comments in the
  seeded tree. It counts the whole tree, so collapsing doesn't change it.

**Comment focus.** When a post is opened, no comment is focused. `n` focuses
the next visible comment in display order (the first one when nothing is
focused), and `p` the previous one (nothing when nothing is focused). Both
stop at the ends, and do nothing on a post without seeded comments. The
focused comment's header row (`u/<author> · <age>`, and the `[+]` and reply
count when collapsed) is drawn like the active nav entry: orange on the
highlight background, bold. `Esc` clears the focus and stays in the post; the
next `Esc` goes back to the feed. `q` and `Backspace` always go back.

**Comment votes.** With a comment focused, `a`/`z` vote on it instead of the
post, with the same arithmetic (see Selection behavior). An upvote draws its
`▲` and score orange, a downvote its `▼` and score periwinkle; with no vote
they stay dim. Scores of 1,000 or more are compacted, as on cards. Voting
never reorders comments ("Sort by: Best" is decorative). Votes are stored on
the `Comment`, so they last for the session like post votes, and are saved
on quit like them (see Persistence). `s` and `c`
always act on the post.

**Collapsing.** `Enter` toggles collapse on the focused comment, and does
nothing with no focus. A collapsed comment is two rows: its header with a
`[+] ` prefix before `u/<author>`, ending in ` (N replies)` (` (1 reply)`, or
nothing for none) where N is `Comment::total() - 1`, and its trailing guide
row. Its body, action row and every descendant are hidden, and `n`/`p` skip
the descendants. Expanded comments carry no marker. Each comment keeps its
own state, so a collapsed reply is still collapsed after its parent is
collapsed and expanded again. Focus and collapse state reset every time a
post is opened from the feed; votes don't.

**Keeping the focus on screen.** After `n` or `p` moves the focus, the next
render scrolls by the least amount that brings the focused comment's header
row into view, up or down (none if it is already visible). It happens once:
`n`/`p` set `Thread.reveal` and the renderer clears it, so `j`/`k`/`PgUp`/
`PgDn`/`Space`/`g`/`G` then scroll freely. `n` or `p` stopped at an end
doesn't move the focus and doesn't scroll. Collapsing doesn't scroll either:
it only removes rows below the focused header, so a header on screen stays
on screen even when the scroll is clamped to the shorter content.

Free-text search terms are highlighted in the title and body the same way as
on the cards, whenever a query is set (so a post opened from a searched feed
stays highlighted), whether or not a comment is focused. The meta row and
comments, `[+]` headers included, are not highlighted, and highlighting
doesn't change the content height or the scroll clamping.

The renderer clamps the scroll position to the content height (after any
collapse) and writes it back into `Mode::Post { scroll }`. So `G` followed by
`k` moves right away, instead of first counting down from `u16::MAX`.

## Data model (`src/data.rs`)

```rust
struct Group   { name, description, members, online, joined: bool }
struct Comment { author, body, score, hours_ago, replies: Vec<Comment>,
                 vote: Vote }
struct Post    { group: usize, author, title, body, flair: Option<&str>,
                 score, hours_ago, comment_count: u32,
                 comments: Vec<Comment>, vote: Vote, saved: bool,
                 hidden: bool }
enum Vote { Up, None /* default */, Down }
```

- **`comment_count` is the displayed total and `comments` is only a sample.**
  A card may say "194 Comments" while the tree holds 5, just as most of a
  Reddit thread sits behind "load more". Don't "fix" the count to
  `comments.len()` or `Comment::total()`. The post view shows the difference
  as "N more comments".
- **Posts have no ID.** While running, they're identified by their index in
  `App.posts`, which never changes after startup. Indices shift whenever the
  seed changes, so the state file names a post by its content instead (see
  Persistence).
- **Comments have no ID either.** The post view identifies a comment by its
  path (`CommentPath` in `app.rs`): its index among the top-level comments,
  then among that comment's replies, and so on. The tree never changes while
  running, so a path always names the same comment. The state file names a
  comment by content, not by path. `Thread::visible` is the only walk
  over the tree. It lists the visible comments in display order, with their
  paths and collapse state, and both `n`/`p` and the renderer use it. A
  vote just follows the focused path down the tree.
- **Comments are built with `c(…)`** (and `.with(replies)`), which sets
  `vote` to `Vote::None`. The tests use the same helpers.
- **Ages:** stored as `hours_ago`, not timestamps, so ordering and the tests
  are deterministic.
- **Groups:** 8 groups. Six start joined: Engineering, RemoteWork, OfficeLife,
  Hackathon, Foodies and OncallSurvivors. Announcements and DogsOfTheOffice
  start unjoined, so their posts appear in Popular, All and their own group
  feed, but not in Home. `J` on a group's feed joins or leaves it.
- **Posts:** 24, built as `Post { …, ..Default::default() }` literals. Every
  name in the seed data is invented. None starts saved or hidden.
- **`hidden`** decides which feeds list the post (see Feeds) and keeps it out
  of Trending today. Its score, votes, comments and save work as usual while
  it is hidden.

## App state (`src/app.rs`)

| Field | Meaning |
|---|---|
| `groups`, `posts` | seed data, with the state file applied by `main` before the first frame. `posts` is mutated in place by votes, saves and hides, `groups[g].joined` by `J` |
| `view: Vec<usize>` | indices into `posts` after feed filter, search and sort. Changed only by a rebuild, except that `h` removes the card it takes out of the feed (see Selection behavior) |
| `selected` | index into `view` |
| `offset` | first card (or compact row) drawn. Maintained by the renderer (see Scrolling) |
| `density: Density` | `Card` (the default) or `Compact`. Toggled by `v` and the view switch, never by a rebuild, and not saved |
| `sort: Sort` | `Hot`, `New`, `Top`, `Rising` |
| `feed: Feed` | `Home`, `Popular`, `All`, `Saved`, `Hidden`, `Group(usize)` |
| `mode: Mode` | `Feed`, `Search`, `Post { scroll: u16 }`. `scroll` is clamped, and moved to reveal a newly focused comment, by the renderer |
| `query: String` | search text, raw as typed. Kept after leaving search mode with Enter |
| `search: Query` | `query` parsed into free-text terms and filters. Re-parsed on every rebuild of `view`, and used both to filter and to highlight |
| `hits: Option<Hits>` | where the most recent frame drew each clickable thing, written by the renderer on every frame. `None` until the first frame (see Mouse) |
| `thread: Thread` | the open post's comment state: `focus: Option<CommentPath>`, `collapsed: HashSet<CommentPath>` (see Data model) and `reveal: bool` (set when `focus` moves, cleared by the renderer once it has scrolled). Reset to the default whenever a post is opened. Only read in the post view |
| `status: Option<&'static str>` | toast text. Every toast is a fixed string. Cleared on the next key press, left click or wheel event |
| `karma: u32` | fixed at 1,337 (shown as `1.3k`). Votes don't change it |
| `should_quit` | checked by the event loop in `main.rs`, and by `main`, which saves the state file only when it is set |

### Feeds (which posts are shown)

A hidden post is in the Hidden feed and in no other: every feed below but
Hidden first leaves out hidden posts, then applies its own rule.

- **Home:** posts from the groups that are joined when it is rebuilt. `J`
  only works on a group feed, so Home is never on screen while membership
  changes. Going back to it rebuilds it with the new membership.
- **Popular:** posts with score ≥ 500, under whatever sort is active. It is a
  filter only and doesn't force Hot.
- **All:** every post.
- **Saved:** posts with `saved` set, from any group, joined or not, that
  aren't hidden.
- **Hidden:** every post with `hidden` set, saved or not, from any group.
  Sorts and search apply as in any other feed.
- **Group(g):** posts in group `g`, whether or not you've joined it.

`[` and `]` cycle Home → Popular → All → Saved → Hidden → each group in seed
order, wrapping in both directions.

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

The query is parsed once per rebuild by `Query::parse` in `app.rs`, into
`App.search`. The same `Query` filters the feed (`Query::matches`) and tells
the renderer what to highlight (`Query::highlights`). It combines with the
current feed and sort, and the view updates on every keystroke.

- **Terms:** the query splits on whitespace, except inside double quotes.
  `"dead code"` is one phrase term. An unclosed quote runs to the end of the
  query, so typing `"dead co` already filters by `dead co`. A term that is
  empty or only whitespace, such as `""` or `" "`, is ignored. A phrase with
  real text keeps its leading or trailing space, so `"dead "` needs the
  space after "dead". Quote characters are never part of a term.
- **Filters:** an unquoted term starting with `w/`, `u/` or `flair:` (the
  prefix in any case) is a filter. `w/<v>` keeps posts whose group name
  starts with `<v>`, `u/<v>` those whose author does, and `flair:<v>` those
  whose flair does (a post with no flair never matches). Values compare
  case-insensitively and may be quoted: `flair:"looking for"`. A filter with
  an empty or whitespace-only value (`w/`, `flair:""`, `flair:" "`) is
  ignored, so typing `w/` doesn't empty the feed. A term that starts with a quote is always free text: `"w/eng"`
  is a phrase.
- **Combining:** filters of the same kind are ORed (`w/eng w/food`), and
  different kinds are ANDed. Every free-text term must also match.
- **Free text:** each term is a case-insensitive substring of the title,
  body, author or group name. Whitespace runs count as one space, in both
  the term and the text, so a phrase matches across a newline in a body.
- **No terms:** a query with no effective terms (blank, spaces only, or only
  ignored phrases and filters) matches every post in the feed, highlights
  nothing, and doesn't count as a search for the empty-feed message.
- **Folding:** matching works on chars, lowercased one char at a time, and
  each folded char remembers which original chars it came from. That is how
  `Query::highlights` maps a match back onto the original text, including
  non-ASCII text like `jalapeño` or `°F` and chars that lowercase into
  several (`İ`). Only free-text terms are highlighted, not filter values.
- **Rendering:** `highlight_rows` in `ui.rs` walks the original text
  alongside the rows `wrap` made from it. `wrap` only drops whitespace and
  joins a row's words with one space, which stands for the whitespace run it
  replaced. `preview` also says how many leading chars of each row are body
  text, so the `…` of a cut row is never highlighted. A row with no match is
  styled exactly as before.

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
- **Hiding takes the card out at once, without a rebuild.** When `h` makes the
  selected post stop belonging to the feed (hiding it anywhere but Hidden,
  unhiding it in Hidden), its card is removed from `view` right away. Nothing
  is re-sorted: the other cards keep their order, `selected` keeps its index
  so the next card takes the removed one's place (or the new last card is
  selected when the last one went), and `offset` is clamped to `selected`, so
  it is never past it or past the end of `view`. Hiding the only card leaves
  the empty-feed message. A kept search, the sort and the feed stay. The
  renderer then scrolls as usual, and it records the click regions afresh, so
  the next click hits the card now drawn there.
- **Toggling the view doesn't rebuild.** `v` (or a click on the view switch)
  changes only `density`, and shows the toast `Compact view` or `Card view`.
  `view`, `selected`, the sort, the feed and a kept search stay as they are,
  and `offset` is left to the renderer, which keeps the selected post fully
  on screen (see Scrolling). It also drops the last frame's click regions,
  which belong to the old view, so a click can only hit what the next frame
  draws. The wheel still works in between. It works in an empty feed too,
  where only the toast and the switch change. Votes, saves, joins and hides
  follow the same rules in both views.
- **Joining and leaving don't rebuild either.** A group feed's posts don't
  depend on membership, so `J` keeps `view` and `selected` as they are, and
  a kept search stays too.
- **Comment votes follow the same rules.** Posts and comments share one
  function for the arithmetic, and comments are never reordered.
- **These rules differ on purpose.** You change feed, sort or search to get a
  new list, so landing at its top is expected. You vote on a card while
  looking at it, so it should stay put. Hiding is different again: you hide a
  post because you don't want to see it, so leaving its card on screen until
  the next rebuild (as an unsave does) would defeat the point, while a full
  rebuild would throw away your place in the feed.

## Key bindings

**Global:** Ctrl-C quits from any mode, including search. Like `q` in the
feed, it saves the state file (see Persistence).

| Key | Feed | Post view |
|---|---|---|
| `j`/`↓`, `k`/`↑` | select next / prev | scroll 1 line |
| `PgDn` / `PgUp` | move 5 posts (cards or rows) | scroll 10 lines |
| `Space` | — | scroll 10 lines |
| `g`/`Home`, `G`/`End` | first / last card | top / bottom |
| `a` / `z` | upvote / downvote | same, for the focused comment, or the post with none focused |
| `n` / `p` | — | focus next / previous visible comment |
| `s` | save / unsave (toast) | same, always for the post |
| `h` | hide / unhide (toast: `Post hidden` / `Post unhidden`). Does nothing in an empty feed | — |
| `J` | join / leave the group (toast). Only on a group feed, with or without a kept search | — |
| `v` | switch between the Card and Compact views (toast: `Compact view` / `Card view`). Works in every feed, an empty one included, and with a kept search | — |
| `c` | "Link copied to clipboard!" toast | same, always for the post |
| `Enter` | open post (does nothing in an empty feed) | collapse / expand the focused comment (nothing with none focused) |
| `1`-`4` / `Tab` | set / cycle sort | — |
| `[` / `]` | prev / next feed | — |
| `/` | enter search mode, editing any kept query | — |
| `Esc` | clear a kept search. Otherwise nothing, so it never quits | unfocus the comment, or back to feed with none focused |
| `q` | quit | back to feed |
| `Backspace` | — | back to feed |

`—` means the key does nothing. In particular, `J` does nothing in the post
view, even on a post from a group feed. Like sorting and switching feeds, it
acts on the feed, so you go back to the feed to join or leave. `h` does
nothing in the post view either: like `J`, hiding acts on the feed. `v`
neither, and going back to the feed returns to the view you left, with the
same selection. The `···`
on a card's action row stays decorative.

**Search mode:**

- Every printable key, including `q`, `j`, `J`, `h`, `v`, `n`, `p` and
  digits, is typed into the query. Phrases and the `w/`, `u/` and `flair:` filters are
  described under Search.
- Backspace deletes a character.
- Enter returns to the feed and keeps the filter.
- Esc returns to the feed and clears it.

## Mouse

`main.rs` turns mouse capture on after `ratatui::init()` and off again
before `ratatui::restore()`. ratatui's panic hook restores the terminal but
knows nothing of mouse capture, so `main` chains its own hook in front of it
that turns capture off first. A panic never leaves the terminal reporting
mouse events. `App::handles_mouse` says whether a mouse event counts (see
below), and only those go to `App::on_mouse`.

**Which events count.** Only left-button presses and the wheel (up and
down). Releases, drags, moves, right and middle clicks and horizontal scrolls
are ignored completely, so they don't even clear the toast. A left press or
a wheel event clears the toast first, as a key press does, even when it
lands on nothing. In search mode every mouse event is ignored, and so is any
event before the first frame is drawn. `handles_mouse` is that rule, and
`on_mouse` checks it first, so an event it rejects changes no state.

| Where | Feed | Post view |
|---|---|---|
| a card: any cell of its drawn rectangle, borders and the visible part of a clipped card included | select it and open it, like `Enter` | — |
| a card's `▲` / `▼` | select it and vote, like `a` / `z`. Doesn't open the post or re-sort | — |
| a card's `☆ Save` / `★ Saved` | select it and save / unsave (toast), like `s` | — |
| a compact row: any cell of it, the meta and blanks included | select it and open it, like `Enter` | — |
| a compact row's `▲` / `▼` | select it and vote, like `a` / `z`. Doesn't open the post or re-sort | — |
| a compact row's `☆` / `★` | select it and save / unsave (toast), like `s` | — |
| a sort tab, its number or label | like its digit, `1`-`4` | not drawn |
| the view switch (`▦ Card` / `≡ Compact`), where drawn | toggle the view, like `v` | not drawn |
| a nav entry's row (`⊘ Hidden` included) | switch to that feed, as `[`/`]` would landing on it. A kept search stays. The current feed: nothing, so the view and selection stay | — |
| the header search box | enter search mode, like `/` | — |
| a comment's header row (`u/<author> · <age>`, `[+]` headers included), across the post box | not drawn | focus it. If it is already focused, collapse / expand it, like `Enter` |
| the wheel, anywhere | select the next / previous card or row, like `j` / `k` | scroll 3 lines, clamped like `j` / `k` |

`—` means the click does nothing. So do the `GROUPS` heading, the blank nav
row, every border outside a card, the logo, the user and karma, the sidebar
(its Join button included) and the footer. In the post view only comment
headers are clickable: bodies, action rows, the post itself, the nav and the
search box do nothing. The nav isn't drawn below 70 columns, and then
nothing in it is clickable.

A click never scrolls. Focusing a comment by clicking it doesn't set
`Thread.reveal`, since its header is already on screen. Collapsing by click
works like `Enter`: it only removes rows below the header, and the renderer
re-clamps the scroll to the shorter content.

**Click regions.** On every frame the renderer records what it drew where in
`App.hits` (`Hits` in `app.rs`): `regions`, a list of `(Rect, Target)` in
drawing order, and `max_scroll`, the post view's scroll clamp (0 outside the
post view). `Target` is `Card(i)`, `Vote(i, Vote)` or `Save(i)` with `i` an
index into `view` (a compact row records the same three), `Sort(Sort)`,
`Density` (the view switch), `Feed(Feed)`, `Search` or
`Comment(CommentPath)`. A click takes the last region containing the cell,
so a card's arrows and Save label, and a compact row's arrows and star,
recorded after the card or row, win over it.

- Regions cover only what was drawn: nav rows only when the nav is drawn and
  only the rows that fit, a clipped card's visible part, arrows and Save
  labels only when their row is drawn, and comment headers only inside the
  scrolled viewport. Everything is clipped like the widgets, so a click
  follows the responsive layout, the feed offset and the post-view scroll.
- The arrows are drawn one cell each, where a centered one-cell line goes,
  so the cell recorded is the cell drawn. The Save label's columns come from
  `actions_line`, which builds the row. A compact row's `▲`, `▼` and star
  columns come from `compact_row`, which builds that row, and each is
  recorded as one cell only when it is inside the row's area. The view
  switch is recorded only when it is drawn, so only in the feed (and in
  search mode, where clicks are ignored).
- `on_mouse` decides what a target does from the mode: card, sort, view
  switch, nav and search targets only act in the feed, `Comment` only in the post view. It
  ignores a target that no longer applies, such as a card index past the end
  of `view` or a comment hidden under a collapsed ancestor, so a stale
  region can't break the focus invariants.
- The wheel in the post view clamps to `max_scroll` straight away, and the
  renderer clamps again as it does for `j`.
- `app.rs` only hit-tests rectangles. It has no rendering code.

**No redraw for ignored events.** Mouse capture turns on any-motion
tracking, so the terminal sends a steady stream of `Moved` events while the
pointer moves. The event loop draws a frame only after an event that may
have changed something: a key press, a mouse event `handles_mouse` accepts,
or any non-mouse event (a resize, for one). A mouse event it rejects changes
nothing, so the frame on screen, and the click regions recorded with it,
are still current. Measured in a 110×24 tmux pane, 300 pointer moves and
a click used to cost 302 full renders, and now cost 2 (startup and the
click). One render takes about 0.3 ms in a release build and 2.6 ms in a
debug build at that size.

## Persistence (`src/state.rs`)

Your vote on each post and on each comment, each post's `saved` and `hidden`
flags and each group's `joined` flag are kept in a small text file between
runs. Nothing else is: every run starts on Home, Hot, at the top, with no
query, focus, collapse or toast.

**What is stored.** Only differences from the seed: a group whose `joined`
differs from the seed's (both joining an unjoined group and leaving a joined
one), each saved post, each hidden post, and each post or comment with a vote.
The seed has no votes, saves or hidden posts (a test checks it). An entry you
undo (vote again, unsave, unhide, re-join) isn't written by the next save, so
it doesn't come back. Scores are not stored: a post's or comment's `score`
already includes your vote, so the file keeps the vote and loading applies it
to the seed score with `cast`'s arithmetic. An upvoted post comes back at
seed + 1, and `a` clears it back to the seed score.

**Keys.** The seed keeps changing, so indices and `CommentPath`s can't be
keys. Every key is built from content instead:

- a group: its `name`;
- a post: its group's `name`, its `author` and its `title`;
- a comment: its post's key, then the `author` and `body` of each comment on
  the way down to it, from the top-level ancestor to the comment itself.

So state follows the same item when groups, posts or sibling comments are
reordered or new ones are inserted anywhere. Changing any field in a key
(renaming a group, editing a title or body, moving a post to another group or
a reply to another parent) makes it a different item, and its old entry stops
matching. A record whose key matches nothing in the current seed is dropped
without an error, and the next save leaves it out. No hashing is involved,
so keys don't depend on the Rust release.

**Shared keys.** Two items could share a key, say two posts with the same
group, author and title, or two sibling replies with the same author and body.
Then the first in seed order (groups, then each post followed by its comments
in pre-order) owns the key. The others are never written or loaded: their
votes, saves and hides last for the session only. A test checks that every key
in the real seed is unique, so this only matters if a duplicate is added.

**Format.** UTF-8 text. The first line is the version, `workit-state 1` or
`workit-state 2` (see Versions). Every other line is one record: tab-separated
fields, ending in a newline. In each field, `\` is written `\\`, a tab `\t`, a
newline `\n` and a carriage return `\r`, and everything else as is (`=` and
non-ASCII included), so any string round-trips. The records, in seed order:

```
workit-state 1
group	left	Engineering
group	joined	Announcements
vote	up	Engineering	dana_k	We deleted 40k lines of dead code and the build got 30% faster
vote	down	Engineering	dana_k	We deleted 40k lines of dead code and the build got 30% faster	lin_tran	212 diffs?! Who reviewed all of these, and are they okay?	oz_reviewer	I have seen things you people wouldn't believe.
saved	OfficeLife	priya_r	PSA: the espresso machine on floor 3 is FIXED
```

- `group`, `joined` or `left`, the group's name.
- `saved`, then the post's key (3 fields).
- `hidden`, then the post's key (3 fields). Only in a `workit-state 2` file.
- `vote`, `up` or `down`, then the key of a post (3 fields) or a comment
  (3 + 2 per level).

A post's records come in the order `saved`, `hidden`, then its `vote`,
followed by its comments' votes. With a hidden post the same file reads:

```
workit-state 2
group	left	Engineering
group	joined	Announcements
vote	up	Engineering	dana_k	We deleted 40k lines of dead code and the build got 30% faster
vote	down	Engineering	dana_k	We deleted 40k lines of dead code and the build got 30% faster	lin_tran	212 diffs?! Who reviewed all of these, and are they okay?	oz_reviewer	I have seen things you people wouldn't believe.
saved	OfficeLife	priya_r	PSA: the espresso machine on floor 3 is FIXED
hidden	OfficeLife	priya_r	PSA: the espresso machine on floor 3 is FIXED
```

**Versions.** A build from before hiding existed reads only `workit-state 1`,
skips a record kind it doesn't know as malformed, and then drops it on its
next save. So that such a build can never silently delete hidden posts, a save
writes `workit-state 2` when it writes at least one `hidden` record, and
`workit-state 1` otherwise. An older build then refuses a file with hidden
posts and leaves it alone, while someone who never hides a post can still go
back to it. Unhiding the last hidden post brings the file back to
`workit-state 1` on the next save. Both versions load. They differ only in
`hidden`: in a `workit-state 1` file a `hidden` line is malformed (skipped and
counted, like any bad line). A `workit-state 1` file from before hiding loads
exactly as before, and is written back byte for byte when nothing changed.

**Location.** `$WORKIT_STATE` if it is set and non-empty, used as given.
Otherwise `$XDG_STATE_HOME/workit/state` when `XDG_STATE_HOME` is an absolute
path. Otherwise `$HOME/.local/state/workit/state` when `HOME` is an absolute
path. When none applies, the app runs without persistence. `state::path`
takes the three values as arguments, so the tests never touch the process
environment.

**When.** `main` builds the `App` with `App::new()` and loads the file into it
before `ratatui::init()`, so before the first frame. Loading applies each
record as the keys would (`cast` for votes), then rebuilds `view`, so restored
state behaves as if you had made those changes by hand: a restored membership
is in Home's first build, a restored save is in Saved, and a restored hidden
post is only in Hidden. `App::new()` and `App::with_data` stay pure and never
touch the file. Saving happens after `ratatui::restore()`, and only when
`should_quit` is set, by `q` in the feed or Ctrl-C in any mode. A panic, or an
I/O error that ends the event loop, doesn't save. When nothing differs from
the seed and there is no file, none is created. Otherwise the whole file is
rewritten atomically: missing parent directories are created, the new contents
go to a temp file `.<name>.<pid>.tmp` (`.state.<pid>.tmp` by default) in the
same directory and are synced, and the temp file is renamed over the old one.
When a regular file is already there (after following symlinks), the temp file
gets its permissions before anything is written to it, so a `0600` file stays
`0600`. A new file gets the default mode. The file always holds either the old
contents or the new ones. A run killed in the middle can leave the temp file
behind, but never a truncated state file. With two instances running at once,
each writes what it saw, and the last one to quit wins.

**Errors.** Bad input never crashes the app or loses the file:

- A missing file means the seed, silently.
- Only a regular file of at most 1 MiB is read (symlinks are followed; the
  seed with every item changed comes to a few dozen KiB). The metadata is
  checked before the file is opened, so a FIFO is never opened and can't
  hang the app before the first frame, and a device such as `/dev/zero`
  can't exhaust memory. The opened file is checked again, and at most 1 MiB
  plus one byte is read, in case it changed in between. Anything else
  (a directory, a FIFO, a device, a bigger file) is handled like a read
  error, below.
- A malformed line (a bad escape, an unknown record kind or value, the wrong
  number of fields, invalid UTF-8, a `hidden` record in a `workit-state 1`
  file) is skipped and the rest still loads. Blank lines are ignored. A record
  repeated in the file sets the same state again, so a vote isn't toggled off.
  The next save drops the bad lines.
- A file whose first line isn't exactly `workit-state 1` or `workit-state 2`
  (empty, garbage, binary, a future `workit-state 3`) loads nothing and is
  never overwritten.
- A read error other than a missing file (permissions, not a regular file,
  over 1 MiB) loads nothing, and that run doesn't save, so a file it couldn't
  read is never replaced.
- On a save error (a directory that can't be created, a full disk, a failed
  rename), the temp file is removed and the old file stays as it was.

Each of these except the missing file is reported as one `workit: …` line on
stderr after the terminal is restored, never in the TUI, and the exit carries
on normally.

## Files

```
workit/
  PLAN.md
  Cargo.toml    edition 2024, depends on ratatui
  src/
    main.rs     ratatui::init()/restore() (restores the terminal on panic), mouse
                capture (also turned off on panic), event loop (no redraw for
                ignored mouse events), loading the state file before the first
                frame and saving it after the restore, messages on stderr
    app.rs      state, feeds, sorts, voting, saving, hiding, joining, the
                Card/Compact toggle, search
                (query parsing, matching, highlight positions), comment focus
                and collapse, key handling, mouse handling (hit-testing the
                click regions). No rendering
    data.rs     seed groups, posts, comments
    state.rs    the state file: content keys, format, loading, atomic saving,
                path selection
    ui.rs       all rendering (cards and compact rows, the view switch), including
                offset upkeep, post-view scroll clamping, revealing the focused
                comment and recording the click regions
```

## Testing

140 unit tests, run with `cargo test`:

- **`app.rs`** (49), on small custom data sets:
  - Home hides unjoined groups, and Popular hides low scores.
  - The exact order for each sort.
  - Feed cycling wraps in both directions, with Saved and then Hidden between
    All and the groups.
  - Joining a group adds its posts to Home and leaving one removes them. `J`
    on a group feed keeps `view` and `selected`, and a kept search. It does
    nothing on Home, Popular, All, Saved and Hidden or in the post view, and
    is typed into the query in search mode.
  - Saved lists exactly the saved posts, combined with a search and with
    Rising. Unsaving there, from the card or from the post view, keeps the
    card until the next rebuild.
  - Hiding in Home, All, a group feed and Saved removing exactly that card,
    the others keeping their order and `selected` its index; a kept search and
    the sort staying; hiding the last card selecting the new last one; hiding
    the only card emptying the feed, after which `h` and Enter do nothing.
  - `h` doing nothing in an empty Hidden feed and in the post view, and being
    typed in search mode.
  - Hidden posts, saved or not and from any group, listed in Hidden and in no
    other feed.
  - Unhiding in Hidden removing the card at once, and the post back in its
    group and in Saved at the next rebuild. Sorting, votes, saves and the post
    view working on hidden posts.
  - Vote toggling and flipping, without reordering the feed.
  - Selection clamping.
  - Search: live filtering, typing `q` doesn't quit, Esc clears.
  - Esc in the feed never quits.
  - Opening and leaving the post view, and Enter in an empty feed.
  - The query tokenizer: whitespace runs, phrases, an unclosed quote, `""`,
    filter prefixes in mixed case, `flair:"looking for"`, a quoted `"w/eng"`
    staying free text, and empty filter values being dropped.
  - Free-text terms ANDing even when not adjacent (`dead faster`), and a
    term that matches nothing emptying the feed.
  - A phrase matching across a newline in a body.
  - `w/` and `u/` prefix filters ORing within a kind and ANDing across kinds.
  - `flair:` never matching a post without a flair.
  - Blank queries and ignored filters matching every post in the feed.
  - Whitespace-only phrases and filter values ignored (they match every post
    and highlight nothing), while `"dead "` keeps its trailing space.
  - `Query::highlights`: overlapping terms, whitespace runs, filters not
    marked, non-ASCII text, and `İ` (which lowercases into two chars).
  - Comments, on a tree four levels deep: `n`, `p` and `Enter` doing
    nothing (and not panicking) on a post without comments; pre-order focus
    stopping at both ends, `p` with no focus doing nothing, and a reveal
    asked for only when the focus moves; `n`/`p` skipping a collapsed
    comment's replies, and a reply staying collapsed while its parent is
    collapsed and expanded; comment vote toggling and flipping without
    touching the post or the order; comment votes kept across reopening
    while focus and collapse reset; `Esc` unfocusing before leaving, and `q`
    and `Backspace` leaving while focused; `s` and `c` acting on the post
    while focused, and `Enter` with no focus doing nothing; `n` and `p`
    doing nothing in the feed and being typed in search mode.
  - Mouse events before the first frame doing nothing, toast included, and
    stale regions (a card past the end of `view`, a comment hidden under a
    collapsed one) being ignored.
  - `v` toggling Card and Compact with the `Compact view` / `Card view`
    toasts, keeping `view`, `selected`, the sort, the feed and a kept
    search; doing nothing in the post view, going back returning to the
    Compact view, and being typed in search mode; working in an empty
    Hidden feed.
  - A toggle dropping the old view's click regions (a stale card region
    opens nothing) while the wheel still works.
  - Hiding in the Compact view removing exactly that post, `selected`
    keeping its index.
- **`ui.rs`** (68), rendering into a `TestBackend`:
  - The helpers `compact`, `age`, `wrap` and `truncate`.
  - The frontpage contents.
  - Sidebars hidden at narrow widths.
  - Sidebar boxes skipped, not squeezed, at 24 rows.
  - The meta row's shortening order, and that it never overflows at any
    width.
  - The selected card staying visible after `G`.
  - Both empty-feed messages, and the empty-Saved one (but not when a search
    or Rising is what empties Saved, or when every saved post is hidden).
  - Hidden's "No hidden posts" with nothing hidden, but "Nothing here yet"
    when Rising filters every hidden post out and "couldn't find any results"
    for a search, live and kept.
  - Hiding cards near the bottom of a scrolled feed, one by one down to none,
    rendering after every key: `offset` never past `selected` or the end of
    `view`, the selected card always drawn, and the post count dropping by one
    each time. Hiding every card of a group feed ending on `0 posts` and
    "Nothing here yet".
  - A hidden post leaving Trending today, the 5th most commented post taking
    its place.
  - Comments in the post view, and scroll clamping.
  - `☆ Saved` right below `◎ All` in the nav, and Saved's sidebar.
  - `⊘ Hidden` right below `☆ Saved`, a click on its row switching to Hidden
    with a kept search staying, and Hidden's sidebar.
  - The group sidebar button and nav mark flipping with `J`.
  - Every hint set (feed, kept search, search, post view unfocused and
    focused) fitting in 80 columns on every feed, Hidden included, in both
    the Card and Compact views, with the exact plain and group feed sets,
    `h hide` and `v view` in both, `J join` only in the feed set on group
    feeds, the exact search-mode and post-view sets, the toggle's toast, a
    toast replacing the focused set, and `Esc` bringing back the unfocused
    one.
  - Highlight cells: a title match black on orange and still bold, in the
    original case, with its neighbors untouched, in a kept search too.
  - A phrase wrapping across two title rows highlighted on both.
  - Overlapping terms drawn as one run.
  - `°f` on a card and `jalapeño` in the post view body highlighting exactly
    their cells, while the comment mentioning it stays plain. `İ` in a post
    and a query doesn't panic.
  - A match cut by the preview's `…` highlighted only in its visible part.
  - Filters alone rendering exactly, styles included, like no query, in the
    feed and in the post view.
  - Highlighting never changing the text on screen, the feed offset, or the
    post-view scroll clamp.
  - A whitespace-only phrase rendering like no query, with no highlights.
  - The empty-feed message ignoring a query with no effective terms (`w/`,
    spaces, `" "`), live and kept, in empty Saved and under Rising, while a
    real filter that finds nothing still says "couldn't find any results".
  - Collapsed comments: `[+]` headers ending in ` (2 replies)`,
    ` (1 reply)` and nothing, each followed by its guide row, with bodies,
    action rows and nested replies gone, no marker on expanded comments, and
    the focused header (only) in the focus style.
  - Comment vote colors (orange up, periwinkle down, dim otherwise, `1.2k`
    compacted) and the post's score left alone.
  - "N more comments" unchanged by collapsing, and a collapse re-clamping
    the scroll so `k` moves right away.
  - The focused comment scrolled into view by the least amount, down and
    up, not at all when already visible, and not pulled back after `k`.
  - A kept search still highlighting the title with a comment focused,
    while the comment, its header and its `[+]` header stay plain.
  - The seed data: the first Home post's three top-level comments
    collapsed by key presses alone, with the exact `[+] u/marco_p · 3h ago
    (1 reply)`, `[+] u/lin_tran · 2h ago (2 replies)` and
    `[+] u/sam_builds · 1h ago` headers, and `··· 188 more comments` before
    and after.
  - Tiny post views (40×12 and 30×8, no sidebars) on a thread five levels
    deep, rendering after every key: walking `n` to the end and `p` back,
    collapsing and expanding, voting on the deepest reply, and `g`/`G`/`k`,
    with the focused header on screen after every `n`/`p` and `scroll`
    always within the clamp.
  - Mouse, with coordinates found in the rendered buffer and a render
    between events, as in the event loop:
    - A card click after scrolling opening `view[offset]`, not `view[0]`,
      and a border cell counting.
    - The visible part of a card clipped at the bottom, down to its two
      border rows, opening it.
    - `▲` and `▼` selecting the card and voting, flipping and clearing,
      without opening or re-sorting. The blank cell beside an arrow opens
      the card.
    - Save and Saved toggling with the toast, without opening.
    - Sort tabs by label and by digit, the current tab resetting the
      selection, and the gap between tabs doing nothing.
    - Nav clicks anywhere on an entry's row, a kept search staying, the
      current feed keeping view and selection, and the heading, blank row
      and border doing nothing. At 60 columns no nav region exists.
    - The search box entering search mode, editing a kept query.
    - Every mouse event in search mode leaving the query, view, feed and
      mode alone.
    - Moves, releases, drags and right and middle clicks leaving the toast,
      and a left click on nothing or a wheel event clearing it.
    - The wheel moving the feed selection from anywhere, clamped at both
      ends, and scrolling the post view by 3, clamped at both ends.
    - Focusing a comment by its header after scrolling, the scroll staying
      put, and bodies, action rows, guides, the post, the nav, the tabs
      area and the search box doing nothing in the post view.
    - Clicking the focused header collapsing it and its `[+]` header
      expanding it, and a collapsed unfocused header being focused, not
      expanded.
    - Tabs, nav and search box working in an empty feed, where the message
      isn't clickable.
    - After hiding the first card, its `▲` and the card itself acting on the
      card now drawn there, never the hidden one.
    - A compact row click after scrolling opening `view[offset]`, its meta
      and blank cells counting, Esc returning to the Compact view with the
      same selection; `▲` and `▼` selecting the row and voting and flipping
      without opening or re-sorting; `☆` saving with the toast and `★`
      unsaving.
    - After a toggle, a click on a cell where the old view drew a card's `▲`
      (or a row's `☆`) acting on what the new view draws there.
    - The wheel moving the selection in the Compact view, clamped, with the
      selected row on screen.
    - The view switch: `▦ Card` and `≡ Compact` at their exact columns at
      110 wide, a click on each toggling with the toast; at 46 columns
      `≡ Compact` drawn one blank after `Rising` and at 45 neither drawn nor
      clickable (`▦ Card`: 43 and 42), and never more than one switch
      region at any width; nothing drawn or clickable in the post view.
  - Compact rows:
    - The exact text of the first two Home rows at 140 columns, one upvoted
      and saved (`★`, orange `▲`, score and star) and one downvoted
      (periwinkle `▼` and score), dim arrows and a white score otherwise, a
      dim meta, and the selected row on `HIGHLIGHT_BG` across its width
      with an orange bold title.
    - The meta dropping the group at 65 columns, the comment count at 49
      and everything at 41, never coming back as the width shrinks, the
      title keeping 20 columns down to 34 and cut below that, no `…` with
      no room, and no row of any seed post wider than its area at any width
      from 1 to 120. Rendering every width from 1 to 120, every arrow and
      star region one cell inside its row, on the glyph drawn.
    - The arrows, star and title in the same columns for scores 1.2k, 845,
      7 and -12.
    - Title highlighting: a match cut by the `…` highlighted only in its
      visible part, the `…` keeping the selected title style, the meta
      plain, the text unchanged; every occurrence in the original case,
      overlapping terms as one run, in a kept search too. Filters alone
      rendering exactly like no query.
    - 24 rows showing 17 posts against 2 cards' arrows, and the selected row
      on screen in the selected style after `G` (offset 2), `k`, `g`, each
      `PgDn` and `PgUp` (5 posts) and `k` all the way up.
    - Switching from Compact to Card with the last post selected scrolling
      on the first frame until that card is whole, bottom border included.
    - Hiding the second row removing exactly it, the next row moving up into
      its place, and the post count dropping by one.
    - `▦` and `≡` one cell wide in ratatui and found nowhere else on the
      Card, Compact and post view screens.
    - `handles_mouse` accepting exactly left presses and the vertical
      wheel, in the feed and the post view, and nothing in search mode or
      before the first frame. Every event it rejects (moves, releases,
      drags, right and middle buttons, horizontal scroll) leaves every
      field of `App` unchanged, the toast included.
- **`state.rs`** (23), on in-memory strings and on files in a directory
  under the system temp dir that each test creates and removes itself. No
  test reads or writes a real state file or reads `HOME`, `XDG_STATE_HOME`
  or `WORKIT_STATE`:
  - A round trip, by key presses, of a post upvote and downvote, a comment
    downvote at depth 2 and an upvote at depth 0, a save, joining an
    unjoined group and leaving a joined one: the exact file contents, the
    exact restored scores (seed ± 1), untouched neighbors, Home's first
    build with the restored membership and Saved with the restored save.
  - A saved upvote, loaded and undone with `a`, giving the seed score and a
    file with no entries; undoing every change leaving a file with only the
    version line; no file, and no directory, created when nothing changed.
  - Keys surviving a seed whose groups, posts and comments are reordered,
    with a new group, a new post and new sibling comments at depths 0 and 2:
    the right items restored, the new ones untouched, and the records written
    back in the new seed order.
  - Keys matching nothing (an unknown group, post or comment, a reply named
    as top-level) dropped without counting as malformed.
  - Keys holding a tab, a newline, a carriage return, backslashes, `=` and
    non-ASCII text round-tripping, next to a post whose title is the first
    one's escaped form, which stays untouched.
  - Every kind of malformed line (bad escape, trailing `\`, unknown kind and
    vote value, missing or extra fields, invalid UTF-8) skipped and counted
    while valid lines load, a repeated vote not toggling, and a last line
    with no newline.
  - An empty, blank, garbage, binary, `workit-state 3`, `workit-state 2 ` or
    `workit-state 1 ` (trailing space) first line loading nothing and the file
    left byte for byte as it was after a save, and a `workit-state 3` file
    with a `hidden` record left alone too, its message naming both versions.
  - A round trip, by key presses, of a hidden post that is also saved and
    upvoted, next to a downvote: the exact file with `workit-state 2` and the
    records in seed order, the post restored hidden (missing from Home and
    Saved, listed in Hidden), and the file written back the same.
  - Unhiding the only hidden post bringing the file back to `workit-state 1`
    with the other records, and the post back on Home.
  - A `hidden` line in a `workit-state 1` file skipped and counted while the
    rest loads, and dropped by the next save; the same line loading in a
    `workit-state 2` file, where a `hidden` record with the wrong fields is
    still malformed and one matching nothing is dropped.
  - A `workit-state 1` file written back byte for byte when nothing changed.
  - A directory at the path: reported, nothing written on save; and as a
    save error, reported with no temp file left.
  - A directory, `/dev/zero`, a FIFO and a symlink to the FIFO each refused
    without blocking: reported, nothing loaded, nothing written. The FIFO
    comes from `mkfifo`, and that case is skipped when `mkfifo` isn't
    available. A symlink to a regular file loads.
  - A file of exactly 1 MiB loading, and one byte more refused and left as
    it was.
  - A save keeping the old file's `0600`, `0640` and `0400` modes, and a new
    file getting the default.
  - Path selection from the three values, including an empty
    `WORKIT_STATE`, a relative `XDG_STATE_HOME` and a relative or missing
    `HOME`.
  - A save creating missing directories and leaving no temp file behind.
  - A shared key belonging to the first item, for posts and comments.
  - Every key in the real seed unique, and the seed with no votes, saves or
    hidden posts.

Lint with `arc f`, `arc lint` and `arc rust-clippy`. The `arc lint -e extra`
engine isn't configured for `users/`.
