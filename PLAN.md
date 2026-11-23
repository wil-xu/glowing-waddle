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
  read its comment tree (voting on comments and collapsing threads),
  commenting on a post and replying to any comment (yours included), saving
  posts and listing them in a Saved feed, and hiding posts, which then show up
  only in a Hidden feed where they can be unhidden.
- Like Reddit, the feed has a Card view (a bordered card per post, the
  default) and a Compact view (one row per post), switched at runtime with
  `v` or the switch on the sort-tab row.
- Like Reddit and most keyboard-driven apps, `?` opens a sheet of every key
  binding and what the mouse does, over the screen it was opened from (see
  Help overlay). The footer only has room for some of them.
- Votes, saves, hidden posts and group membership survive a restart (see
  Persistence).
- One self-contained binary. No network and no real Workplace data: all
  content is hard-coded, fictional, and safe for work.

## Non-goals

- Fetching real Workplace content (would need auth and APIs, and raises privacy
  concerns).
- Persisting anything beyond votes, saves, hidden posts and group membership.
  The feed, sort, search, Card/Compact view, selection, scroll, comment focus
  and collapse state, the help overlay and the toast start fresh on every run,
  on Home, Hot, in the Card view, at the top, with the help closed.
- Keeping the comments you write. They last for the session only: every run
  starts with the seed tree, and nothing about them (their text, your upvote
  on them, any vote you cast on them, the higher comment count) reaches the
  state file. Editing or deleting a comment isn't supported either.

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
 a/z vote  1-4 sort  [/] feed  / search  s save  h hide  ? help  q quit
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
 a/z vote  1-4 sort  [/] feed  / search  s save  h hide  ? help  q quit
```

**No emoji, on purpose.** Every glyph is a single-width character:
`◉ ⌕ ✦ ⌂ ↗ ◎ ● ○ ▲ ▼ ▤ ☆ ★ ⊘ ··· ✔ ▦ ≡ ↓ ↑` (`↓ ↑` only in the help).
Emoji such as 🔥 or 💬 render two cells
wide in most terminals, and widely in tmux, which breaks the alignment of the
bordered cards. Don't add them. Check new symbols too: `☰` (the trigram,
an obvious "list" icon) was tried for the Compact switch and dropped. It is
Wide since Unicode 16, so ratatui (`unicode-width` 0.2) gives it two cells
while tmux 3.7 draws it in one, and the label came out with a gap. A test
checks that `▦` and `≡` are one cell each in ratatui and appear nowhere else
on screen. Another checks every char of the help table and the footer hint
sets the same way, `↓` and `↑` included (tmux 3.7 draws them in one cell
too).

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
  for the feed, the feed with a kept search, search mode, the post view with
  and without a focused comment, the composer and the help overlay. They are
  constants next to `render_footer` (`FEED_HINTS`, `GROUP_HINTS`, ...),
  picked by `hints`. The plain feed set is `a/z vote`, `1-4 sort`, `[/] feed`,
  `/ search`, `s save`, `h hide`, `? help`, `q quit` (72 columns, counting
  the blank after the last hint). On a group feed it adds `J join` before
  `? help` (80 columns, exactly the limit). Every set must fit on one line at
  80 columns on every feed, Hidden and every group included, in both views,
  so the feed sets leave things out, and `?` lists all of them:
  - `enter open`. It was first dropped from the group set alone, when
    `J join` would have made that set 84 columns: Enter is the most
    guessable key, and a click on a card or a compact row opens it too.
    Adding `v view` would have made the plain set 84, so it left out
    `enter open` as well.
  - `v view`. Adding `? help` made the group set 88, so both sets gave up
    `v view` for it. Of the hints left, it is the only one whose action is
    already on screen: the view switch (`▦ Card` / `≡ Compact`) on the tab
    row names the current view and toggles it on a click, and it is drawn at
    80 columns. Every other hint is a key you can't guess, shortening labels
    (`[] feed`, `/ find`) saves only a column or two each, and dropping the
    same hint from both sets keeps them the same apart from `J join`. The
    trade-off: `v` is now found only through the switch or `?`.
  - `j/k` and `c` (which the post view set shows), and `Tab` (only `1-4` for
    sorting).

  The feed set with a kept search is unchanged and still lists `enter open`:
  `j/k move`, `enter open`, `/ edit search`, `esc clear search`, `q quit`.
  The search set is `type filter`, `w/ u/ flair: narrow`, `enter keep`,
  `esc clear`. The post view set is `j/k scroll`, `n/p comments`, `a/z vote`,
  `r reply`, `s save`, `c share`, `? help`, `esc back` (80 columns, exactly
  the limit). With a comment focused it is `n/p comments`,
  `a/z vote comment`, `r reply`, `enter collapse`, `? help`, `esc unfocus`
  (78): with `j/k scroll` it would be 90, and that is the hint to drop, since
  a comment is only ever focused from the unfocused post view, whose set
  shows it, the arrow keys scroll too, and `j`/`k` still work. `vote comment`
  was the other candidate (shortening it to `vote` saves 8), but it is the
  one hint that says the votes now go to the comment rather than the post,
  which you can't guess. The composer's set is `type reply`, `enter post`,
  `esc cancel` when replying to a comment, and `type comment`, `enter post`,
  `esc cancel` when commenting on the post; it has no `? help`, since the
  composer types `?` like any other printable key. While the help is open
  the footer shows `j/k scroll`, `g/G top/bottom`, `?/esc close` (42
  columns, so `close` still shows at 50; the help itself lists `PgDn`, `PgUp`
  and `Space`). The hints are the same in both views. A status toast
  (green, `✔ …`) replaces the hints until the next key press, left click or
  wheel event (see Mouse).
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
  trailing guide line. `Reply` opens the composer (see below, and Mouse).
  `Share` is decorative.
- "··· N more comments", where N is `comment_count` minus the comments in the
  tree. It counts the whole tree, so collapsing doesn't change it. A comment
  you write adds 1 to both, so it doesn't change N either.

**Comment focus.** When a post is opened, no comment is focused. `n` focuses
the next visible comment in display order (the first one when nothing is
focused), and `p` the previous one (nothing when nothing is focused). Both
stop at the ends, and do nothing on a post without comments. The
focused comment's header row (`u/<author> · <age>`, and the `[+]` and reply
count when collapsed) is drawn like the active nav entry: orange on the
highlight background, bold. `Esc` clears the focus and stays in the post; the
next `Esc` goes back to the feed. `q` and `Backspace` always go back (unless
the help is open: then `q` closes it, see Help overlay).

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

**Keeping the focus on screen.** After `n` or `p` moves the focus, the
composer opens (and shrinks the viewport), or a submit focuses your new
comment, the next render scrolls by the least amount that brings the focused
comment's header row into view, up or down (none if it is already visible,
or if nothing is focused). It happens once: `n`/`p`, opening the composer
and the submit set `Thread.reveal` and the renderer clears it, so `j`/`k`/`PgUp`/
`PgDn`/`Space`/`g`/`G` then scroll freely. `n` or `p` stopped at an end
doesn't move the focus and doesn't scroll. Collapsing doesn't scroll either:
it only removes rows below the focused header, so a header on screen stays
on screen even when the scroll is clamped to the shorter content.

**The composer (`r`).** In the post view, `r` opens the composer: with a
comment focused it replies to that comment, with none it writes a top-level
comment on the post. Clicking a comment's `Reply` label does the same for
that comment, focusing it first (see Mouse). `r` does nothing in the feed and
is typed into the query in search mode. The composer is a bordered box
(orange, like the post box) docked at the bottom of the post column, across
its full width, 3 text rows high (5 with the border). Its title says what it
answers, ` Replying to u/<author> ` or ` Commenting on the post `, and its
bottom border shows, right-aligned, `<n>/500`: the number of chars typed. The
post view keeps working above it as a scrolled viewport in the remaining
height: its scroll clamp, the reveal and its click regions are computed for
the shorter area, exactly as if the terminal were 5 rows shorter. The header,
nav, sidebar and footer stay as they are; only the footer's hints change.
When the column is shorter than 5 rows, the composer takes all of it and is
clipped, and the post view gets none.

The draft is word-wrapped like comment bodies (`wrap`), and a cursor cell
(the search box's orange `▏`) is drawn right after the last char. A trailing
whitespace run shows as one space before the cursor, as `wrap` shows a run
between words, and when the last row is full the cursor starts a new one
(`draft_rows` in `ui.rs`). When the draft needs more than 3 rows, the box
shows the last 3, so the cursor is always visible.

**Keeping what you answer on screen.** The composer takes 5 rows from the
bottom of the post viewport, so a focused comment whose header was in those
rows would end up under it, and you would type a reply to something you
can't see. Opening the composer (by `r` or a click on `Reply`) therefore
asks for the same reveal as `n`/`p`: the first frame with the composer
scrolls the shrunk viewport by the least amount that keeps the focused
header row visible. A header already above the composer doesn't scroll,
and with nothing focused (a top-level comment) nothing scrolls. A header in
the last 5 rows lands on the last row above the composer. Esc must give back
the view you had, so `Mode::Compose` keeps two scrolls: `scroll`, the shrunk
viewport's (moved by that reveal), and `restore`, the post view's scroll
from just before the composer opened. Esc returns to `Mode::Post` with
`restore`, exactly the old scroll, and drops a reveal not yet drawn so it
can't move it. A submit carries `scroll` on, since that is the view you
were looking at, and then reveals the new comment as usual.

While it is open, every printable key is typed into the draft (see Key
bindings), up to 500 chars. Enter submits, Esc cancels: the draft is
discarded and you are back in the post view with the same focus and the
scroll from before it opened. The focus can't move while the composer is
open (no key reaches the post view and the mouse is ignored), so
`Thread.focus` is what it answers.

**Submitting.** Enter on a draft that is empty or only whitespace does
nothing and keeps the composer open. Otherwise the composer closes and the
draft, trimmed at both ends, becomes a new comment (`Comment::yours`):

- by `u/you` (the user in the header), `just now` (`hours_ago` 0), already
  upvoted by you: `vote` `Up` and `score` 1, so `a` clears it to 0 and `z`
  flips it to -1 with the usual `cast` arithmetic;
- placed **first** among its siblings, as Reddit shows your new comment at
  the top: first among the post's top-level comments, or first among the
  focused comment's replies. Replying to a collapsed comment expands it, so
  the reply is visible;
- the post's `comment_count` goes up by 1, so its card shows the new total,
  while `··· N more comments` stays the same;
- the focus moves to the new comment and the next render reveals it once,
  like `n`/`p`;
- toast: `Comment posted` or `Reply posted`.

It works on any post (a hidden one, one with no seeded comments) and on any
comment, your own included, at any depth. Inserting shifts the paths of
later siblings; see Data model for how the focus and collapse state follow.
Your comments behave like any other (focus, votes, collapse, replies) but are
never saved (see Persistence).

Free-text search terms are highlighted in the title and body the same way as
on the cards, whenever a query is set (so a post opened from a searched feed
stays highlighted), whether or not a comment is focused. The meta row and
comments, `[+]` headers included, are not highlighted, and highlighting
doesn't change the content height or the scroll clamping.

The renderer clamps the scroll position to the content height (after any
collapse) and writes it back into `Mode::Post { scroll }`. So `G` followed by
`k` moves right away, instead of first counting down from `u16::MAX`.

### Help overlay (`?`)

`?` in the feed (either view, any feed, an empty one and one with a kept
search included) or in the post view (focused or not) opens a box titled
`Keys` over the body. In search mode `?` is typed into the query like any
other printable key. Captured from the real binary in a 110×24 tmux pane,
right after `?` on the frontpage:

```
╭────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│ ◉  workit     ⌕ Search Home                                                           ● u/you  ✦ 1.3k karma│
╰────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
╭────────────────────╭ Keys ───────────────────────────────────────────────────────────╮─────────────────────╮
│ ⌂ Home             │ Feed                                                            │sonal workit         │
│ ↗ Popular          │   j/k, ↓/↑       select the next / previous post                │e. Come here to      │
│ ◎ All              │   PgDn/PgUp      move 5 posts down / up                         │ with your favorite  │
│ ☆ Saved            │   g/G, Home/End  first / last post                              │                     │
│ ⊘ Hidden           │   a/z            upvote / downvote, again to undo               │                     │
│                    │   s              save / unsave                                  │ Create Post         │
│ GROUPS             │   h              hide / unhide (hidden posts are in Hidden)     │─────────────────────╯
│ ● w/Engineering    │   J              join / leave the group, on a group feed        │ today ──────────────╮
│ ● w/RemoteWork     │   v              switch between the Card and Compact views      │n 2026 is happening… │
│ ● w/OfficeLife     │   c              copy the post's link                           │cements · 545 comme… │
│ ● w/Hackathon      │   enter          open the post                                  │got his own visitor… │
│ ○ w/Announcements  │   1-4            sort by Hot, New, Top, Rising                  │TheOffice · 406 com… │
│ ● w/Foodies        │   Tab            next sort                                      │r opinion: most of … │
│ ○ w/DogsOfTheOffice│   [/]            previous / next feed                           │ering · 393 comments │
│ ● w/OncallSurvivors│   /              search, editing a kept search                  │ding opens Monday: … │
│                    │   esc            clear a kept search                            │cements · 332 comme… │
│                    │   ?              this help                                      │─────────────────────╯
│                    │   q              quit                                           │
╰────────────────────╰───────────────────────────────────────────────────── 1-18 of 70 ╯
 j/k scroll  g/G top/bottom  ?/esc close
```

- **One table.** Everything in the box comes from `HELP` in `ui.rs`: seven
  sections (Feed, Post view, Search mode, Composer, Mouse, This help,
  Anywhere for Ctrl-C), each a list of rows, each row its keys (shown joined by `, `) and
  what they do. It mirrors "Key bindings" and "Mouse" below, so change them
  together. A test fails if a key of any footer hint set is missing from its
  mode's section, so the footer can't name a key the help doesn't explain.
- **Order.** The section for the mode it was opened from comes first (Feed
  or Post view), then the others in table order. It never opens from search
  mode or the composer, which type `?` like any other printable key. A blank line separates
  sections. Headings are white and bold, keys orange and bold, as in the
  footer. Each section's key column is as wide as its longest keys.
- **Layout.** The box is centered over the body, whatever the mode drew
  there, and the header and footer stay. It is as wide as the widest line
  plus borders and a blank column each side (67 columns), or the body if
  narrower, and as tall as the content plus borders, or the body if shorter.
  So it always fits inside the body and never draws over the header or
  footer, and with no body rows it isn't drawn at all. Its cells are cleared
  first, so nothing shows through. Outside it the screen is drawn exactly as
  without the help. At 80 columns and wider no line is cut off. In a
  narrower box descriptions wrap under themselves, and a key wider than the
  box is clipped. It never panics, down to 1×1.
- **Scrolling.** `Help.scroll` is the first line shown. The renderer clamps
  it to the content height and writes it back on every frame, as in the post
  view, so `G` then `k` moves at once, and after a resize the scroll fits the
  new box: never past the last line, and none at all when everything fits. When there is more than fits, the bottom border
  shows the lines in view, `1-18 of 70` at 80×24.
- **Modal.** The help sits on top of the mode and leaves it alone. Closing
  it returns to exactly the screen it was opened from: mode, feed, sort,
  kept search, selection, `offset`, view, post scroll, focus and collapse.
  While it is open:
  - `?`, `Esc` and `q` close it. `q` never quits here.
  - `j`/`↓`, `k`/`↑` scroll 1 line, `PgDn`/`Space` and `PgUp` 10, `g`/`Home`
    and `G`/`End` go to the top and bottom.
  - Every other key does nothing. Voting, saving, hiding, joining, the view
    toggle, sorts, feeds, search, opening, comment focus and collapse never
    reach the screen underneath.
  - Any key press clears a toast, as always, and Ctrl-C still quits and
    saves.
  - The wheel scrolls it 3 lines per notch, anywhere on screen, clamped. A
    left click inside the box does nothing. A left click anywhere else
    closes it and does nothing more, even where the screen underneath drew a
    card, tab, switch, nav entry, search box or comment header (see Mouse).
- **Not saved.** Whether it is open and its scroll are session state only.
  Every run starts with it closed, and the state file doesn't change.

## Data model (`src/data.rs`)

```rust
struct Group   { name, description, members, online, joined: bool }
struct Comment { author: &'static str, body: Cow<'static, str>, score,
                 hours_ago, replies: Vec<Comment>, vote: Vote,
                 mine: bool }
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
  as "N more comments". A comment you write adds 1 to `comment_count` as
  well as to the tree, so the card shows the new total and the difference
  stays the same.
- **Posts have no ID.** While running, they're identified by their index in
  `App.posts`, which never changes after startup. Indices shift whenever the
  seed changes, so the state file names a post by its content instead (see
  Persistence).
- **Comments have no ID either.** The post view identifies a comment by its
  path (`CommentPath` in `app.rs`): its index among the top-level comments,
  then among that comment's replies, and so on. The state file names a
  comment by content, not by path. `Thread::visible` is the only walk
  over the tree. It lists the visible comments in display order, with their
  paths and collapse state, and both `n`/`p` and the renderer use it. A
  vote just follows the focused path down the tree.
- **How paths stay valid.** The only change to a tree while running is a
  comment you write, inserted at index 0 of its parent's list. That moves
  every later sibling up one index, and with it every descendant of one,
  since a descendant's path starts with its ancestor's. Nothing else moves:
  ancestors, earlier siblings and other branches keep their paths. Paths
  are held in only three places: `Thread.focus`, `Thread.collapsed` and the
  last frame's click regions. Before anything reads them again, the submit
  calls `Thread::make_room(at)`, which, for the new comment's path `at` =
  `parent + [i]`, adds 1 at position `parent.len()` to every path that
  starts with `parent` and has an index of `i` or more there. So each
  collapsed comment is still collapsed and drawn as collapsed, and `n`/`p`
  and votes (which only ever follow `focus`) reach the comments you meant.
  Then the parent is removed from `collapsed` (a reply expands it) and the
  focus is set to `at`. The click regions are simply dropped, as a view
  toggle drops them, so a click before the next frame hits nothing; the
  next frame records them afresh. Paths stay plain paths and seed comments
  get no ID: the shift is a few lines in one place, and every other rule
  (pre-order walk, stale-region checks, persistence keys) is unchanged.
- **Comment text.** `author` stays `&'static str`: seed authors are literals
  and yours is the constant `YOU` (`"you"`, the same one the header draws as
  `u/you`). `body` is a `Cow<'static, str>`: `Borrowed` for the seed, so the
  seed allocates nothing and `c(…)` still takes a string literal, and
  `Owned` for a comment typed at runtime, so no text is leaked to fake a
  `'static`. A `String` would have worked too but would copy every seed body
  at startup for no gain. A separate type for your comments would have
  needed a second kind of node in `replies` (or a second tree), and every
  walk, the renderer and the key code to handle both, while yours behave
  exactly like seed comments apart from persistence. `mine` marks them; it
  is what keeps them out of the state file, rather than the author's name,
  so a seed comment by someone called `you` is still saved.
- **Comments are built with `c(…)`** (and `.with(replies)`), which sets
  `vote` to `Vote::None` and `mine` to false, and keeps the seed as readable
  as before. The tests use the same helpers. `Comment::yours(body)` builds
  one of yours: `u/you`, `hours_ago` 0, `vote` `Up`, `score` 1, `mine`.
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
| `groups`, `posts` | seed data, with the state file applied by `main` before the first frame. `posts` is mutated in place by votes, saves, hides and the comments you write (inserted into `comments`, and `comment_count` + 1), `groups[g].joined` by `J` |
| `view: Vec<usize>` | indices into `posts` after feed filter, search and sort. Changed only by a rebuild, except that `h` removes the card it takes out of the feed (see Selection behavior) |
| `selected` | index into `view` |
| `offset` | first card (or compact row) drawn. Maintained by the renderer (see Scrolling) |
| `density: Density` | `Card` (the default) or `Compact`. Toggled by `v` and the view switch, never by a rebuild, and not saved |
| `sort: Sort` | `Hot`, `New`, `Top`, `Rising` |
| `feed: Feed` | `Home`, `Popular`, `All`, `Saved`, `Hidden`, `Group(usize)` |
| `mode: Mode` | `Feed`, `Search`, `Post { scroll: u16 }`, `Compose { scroll: u16, restore: u16 }` (the post view with the composer open; `scroll` is the shrunk viewport's, which a submit carries on, and `restore` the post view's scroll from before it opened, which Esc goes back to). `scroll` is clamped, and moved to reveal a newly focused comment, by the renderer. `Mode` stays `Copy`: the draft lives in `draft` |
| `query: String` | search text, raw as typed. Kept after leaving search mode with Enter |
| `search: Query` | `query` parsed into free-text terms and filters. Re-parsed on every rebuild of `view`, and used both to filter and to highlight |
| `help: Option<Help>` | the `?` overlay: `None` when closed (at startup), else `Help { scroll: u16 }`. While it is open, keys and clicks go to it and `mode` is left as it was. `scroll` is clamped by the renderer. Not saved |
| `hits: Option<Hits>` | where the most recent frame drew each clickable thing, written by the renderer on every frame. `None` until the first frame (see Mouse). A submit empties its regions |
| `draft: String` | the composer's text, raw as typed, at most 500 chars. Emptied when the composer opens, on Esc and on a submit. Only used in `Mode::Compose`. What it answers is `thread.focus`, which can't change while the composer is open |
| `thread: Thread` | the open post's comment state: `focus: Option<CommentPath>`, `collapsed: HashSet<CommentPath>` (see Data model, and how a submit shifts both) and `reveal: bool` (set when `focus` moves by `n`/`p` or a submit, cleared by the renderer once it has scrolled). Reset to the default whenever a post is opened. Only read in the post view and the composer |
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

**Global:** Ctrl-C quits from any mode, including search, the composer and
with the help open. Like `q` in the feed, it saves the state file (see
Persistence).

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
| `r` | — | open the composer: a reply to the focused comment, or a top-level comment with none focused |
| `Enter` | open post (does nothing in an empty feed) | collapse / expand the focused comment (nothing with none focused) |
| `1`-`4` / `Tab` | set / cycle sort | — |
| `[` / `]` | prev / next feed | — |
| `/` | enter search mode, editing any kept query | — |
| `Esc` | clear a kept search. Otherwise nothing, so it never quits | unfocus the comment, or back to feed with none focused |
| `?` | open the help (see Help overlay) | same |
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

- Every printable key, including `q`, `j`, `J`, `h`, `v`, `n`, `p`, `r`, `?` and
  digits, is typed into the query. Phrases and the `w/`, `u/` and `flair:` filters are
  described under Search.
- Known limitation: modifiers are ignored here, so a Ctrl or Alt chord
  (Ctrl-W, Ctrl-U, Alt-x) is typed as its plain letter (`w`, `u`, `x`).
  Ctrl-C still quits. The composer doesn't have this quirk (below); search
  mode was left as it was.
- Backspace deletes a character.
- Enter returns to the feed and keeps the filter.
- Esc returns to the feed and clears it.

**Help overlay** (open over the feed or the post view):

| Key | Help |
|---|---|
| `j`/`↓`, `k`/`↑` | scroll 1 line |
| `PgDn`/`Space`, `PgUp` | scroll 10 lines |
| `g`/`Home`, `G`/`End` | top / bottom |
| `?`, `Esc`, `q` | close, back to exactly where it was opened |
| anything else | nothing (it never reaches the screen underneath) |

**Composer** (`r` or a `Reply` click in the post view):

- Every printable key, including `q`, `j`, `k`, `r`, `n`, `p`, `a`, `z`,
  `s`, `c`, `h`, `v`, `J`, `g`, `G`, `?`, space and digits, is typed into the
  draft, and none of them acts. Only unmodified or Shift-modified keys
  count: Shift is how capitals and symbols arrive. Any key with Ctrl, Alt or
  another modifier (Ctrl-W, Ctrl-U, Alt-x, Ctrl-Backspace, Alt-Enter) does
  nothing, except Ctrl-C, which quits as from any mode. Control characters
  aren't typed. Once the draft has 500 chars (chars, not bytes), further
  printable keys are ignored.
- Backspace deletes one char (`ñ` or `°` included), and does nothing on an
  empty draft.
- There are no newlines: Enter submits (see Submitting), and does nothing
  on a draft that is empty or only whitespace.
- Tab, the arrow keys and every other key do nothing.
- Esc discards the draft and returns to the post view with the same focus
  and the exact scroll from before the composer opened, even if opening it
  scrolled to keep the focused header visible.

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
lands on nothing. In search mode and while the composer is open every mouse
event is ignored, clicks and the wheel included, and so is any event before
the first frame is drawn. `handles_mouse` is that rule, and
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
| a comment's `Reply` label, exactly its 5 cells | not drawn | focus that comment and open the composer for it, in one click, like `r`. On the focused comment it just opens the composer |
| the wheel, anywhere | select the next / previous card or row, like `j` / `k` | scroll 3 lines, clamped like `j` / `k` |

**While the help is open** (feed or post view underneath), the table above
doesn't apply. The wheel scrolls the help 3 lines per notch, clamped like
its `j`/`k`. A left click inside its box, borders included, does nothing.
A left click anywhere else closes it and does nothing more: it never votes,
opens, saves, sorts, toggles the view, switches feeds, enters search or
focuses a comment, whatever the screen underneath drew there. The events
ignored everywhere (releases, moves, drags, right and middle clicks,
horizontal scroll) are ignored here too, and cause no redraw.

`—` means the click does nothing. So do the `GROUPS` heading, the blank nav
row, every border outside a card, the logo, the user and karma, the sidebar
(its Join button included) and the footer. In the post view only comment
headers and `Reply` labels are clickable: bodies, the rest of the action
rows (the arrows, the blanks and `Share`, which stays decorative), the post
itself, the nav and the search box do nothing. The nav isn't drawn below 70 columns, and then
nothing in it is clickable.

A click never scrolls by itself. Focusing a comment by clicking its header
doesn't set `Thread.reveal`, since it is already on screen. A click on
`Reply` opens the composer, and that may scroll, by the least amount that
keeps the comment's header above the composer (see the composer in Post
view). Esc then restores the scroll from before the click. Collapsing by click
works like `Enter`: it only removes rows below the header, and the renderer
re-clamps the scroll to the shorter content.

**Click regions.** On every frame the renderer records what it drew where in
`App.hits` (`Hits` in `app.rs`): `regions`, a list of `(Rect, Target)` in
drawing order, and `max_scroll`, the scroll clamp of what the wheel scrolls
(the help while it is open, else the post view, 0 in the feed). `Target` is
`Card(i)`, `Vote(i, Vote)` or `Save(i)` with `i` an index into `view` (a
compact row records the same three), `Sort(Sort)`, `Density` (the view
switch), `Feed(Feed)`, `Search`, `Comment(CommentPath)`, `Reply(CommentPath)`
or `Help` (the help's box). A click takes the last region containing the cell,
so a card's arrows and Save label, and a compact row's arrows and star,
recorded after the card or row, win over it.

- Regions cover only what was drawn: nav rows only when the nav is drawn and
  only the rows that fit, a clipped card's visible part, arrows and Save
  labels only when their row is drawn, and comment headers and `Reply`
  labels only inside the scrolled viewport (above the composer when it is
  open). Everything is clipped like the widgets, so a click
  follows the responsive layout, the feed offset and the post-view scroll.
- The arrows are drawn one cell each, where a centered one-cell line goes,
  so the cell recorded is the cell drawn. The Save label's columns come from
  `actions_line`, which builds the row. A compact row's `▲`, `▼` and star
  columns come from `compact_row`, which builds that row, and each is
  recorded as one cell only when it is inside the row's area. The view
  switch is recorded only when it is drawn, so only in the feed (and in
  search mode, where clicks are ignored).
- `Reply` is recorded on exactly the cells it is drawn on, in the post view
  only. Its columns come from `comment_lines`, which builds the action row
  (the label is a span of its own, styled as before, so the row renders the
  same), and are clipped to the post box's text area like the row: at a
  narrow width only the visible part of the label is a region, and none
  when it is cut off entirely. Collapsed comments have no action row and so
  no `Reply`. The regions are also recorded while the composer is open, for
  the shorter viewport, though no click reaches them.
- `on_mouse` decides what a target does from the mode: card, sort, view
  switch, nav and search targets only act in the feed, `Comment` and
  `Reply` only in the post view, and nothing in search mode or the
  composer. It ignores a target that no longer applies, such as a card
  index past the end of `view` or a comment hidden under a collapsed
  ancestor, so a stale region can't break the focus invariants. A submit
  shifts paths (see Data model), so it drops the last frame's regions
  rather than leave any that name a path from before the insert.
- The wheel in the post view clamps to `max_scroll` straight away, and the
  renderer clamps again as it does for `j`. So does the wheel in the help
  (before the first frame after `?`, against the stale `max_scroll`, which
  the next frame corrects).
- While the help is open, the renderer draws the screen underneath as
  usual, then drops every region it recorded and records only the help's
  box as `Help`, with the help's `max_scroll`. `on_mouse` handles the help
  before looking at the mode: a click is inside only when it hits `Help`,
  so even a click before the first frame after `?`, while the old regions
  are still there, can only close the help.
- `app.rs` only hit-tests rectangles. It has no rendering code.

**No redraw for ignored events.** Mouse capture turns on any-motion
tracking, so the terminal sends a steady stream of `Moved` events while the
pointer moves. The event loop draws a frame only after an event that may
have changed something: a key press, a mouse event `handles_mouse` accepts,
or any non-mouse event (a resize, for one). `handles_mouse` rejects every
mouse event in search mode and the composer, so those cost no redraw either. A mouse event it rejects changes
nothing, so the frame on screen, and the click regions recorded with it,
are still current. Measured in a 110×24 tmux pane, 300 pointer moves and
a click used to cost 302 full renders, and now cost 2 (startup and the
click). One render takes about 0.3 ms in a release build and 2.6 ms in a
debug build at that size.

## Persistence (`src/state.rs`)

Your vote on each post and on each comment, each post's `saved` and `hidden`
flags and each group's `joined` flag are kept in a small text file between
runs. Nothing else is: every run starts on Home, Hot, at the top, with no
query, focus, collapse or toast, the help closed, and with the seed's comment tree.

**Your comments are never saved.** The comments you write last for the
session only (see Non-goals). `items`, the one list of what can be saved,
skips every comment with `mine` set, and everything under one (which can
only be yours, and whose key would contain your text anyway). From that:

- no record is ever written for one of your comments: not its text, not the
  upvote it starts with, not any vote you cast on it;
- writing comments changes nothing in the file. The records come in seed
  order, and inserting your comments doesn't change the relative order of
  the seeded ones, so with the same votes, saves, hides and joins the file
  is byte for byte what it would be without them. `comment_count` was never
  saved, so the higher count isn't either;
- your comment can never own or shadow a seeded comment's key, even when its
  author and text are exactly a seeded sibling's (a seed author called
  `you`): it isn't in the list the first-owner rule picks from;
- loading never touches your comments: a record can only resolve to an item
  in that list. At load time there are none anyway (`main` loads before the
  first frame), but the code doesn't rely on it.

A seeded comment that one of yours pushed to a later index still saves
under its own key and loads back onto it: the key is built from content
(the authors and bodies on the way down), never from the path, and `items`
pairs each key with the comment's current path when it is built, at save
time. On the next run the comment is back at its seed path, which is the
path `items` finds for that key then.

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
                and collapse, the composer (typing, submitting, inserting your
                comment and shifting the paths), key handling (the help's keys
                included), mouse handling (hit-testing the click regions). No
                rendering
    data.rs     seed groups, posts, comments; `Comment::yours`, `YOU` and the
                500-char limit
    state.rs    the state file: content keys (never for your comments), format,
                loading, atomic saving, path selection
    ui.rs       all rendering (cards and compact rows, the view switch, the
                composer, the help overlay and its table, the footer's hint
                sets), including offset upkeep, post-view and help scroll
                clamping, revealing the focused comment and recording the
                click regions
```

## Testing

197 unit tests, run with `cargo test`:

- **`app.rs`** (73), on small custom data sets:
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
  - The help closed at startup, and `?` opening it, changing nothing else,
    from the Card and Compact views, a group feed, an empty Hidden feed, a
    kept search, and the post view with and without a focused comment (and
    a collapsed one); `?` typed into the query in search mode; `q` closing
    it without quitting (the next `q` quits) and Ctrl-C quitting with it
    open from the feed and the post view; a key press clearing a toast while
    `a` does nothing.
  - The composer: `r` with and without a focused comment opening it for
    that comment or the post, keeping the scroll; `r` doing nothing in the
    feed and typed in search mode; `q`, `j`, `r`, `a`, `z`, `s`, `c`, `h`,
    `v`, `J`, digits and more typed without acting (no quit, no vote, no
    toast, posts and thread unchanged); Backspace deleting `°` and `ñ` one
    char each; the 500-char limit counted in chars, with room again after a
    Backspace; Tab, the arrows and other keys doing nothing; Esc discarding
    the draft and keeping the focus and scroll, and the next composer
    starting empty; Ctrl-C quitting from it.
  - Enter on an empty, a spaces-only and a no-break-space draft doing
    nothing and keeping the composer and the draft.
  - A top-level comment (trimmed) and a nested reply landing first among
    their siblings, by `you`, `just now`, score 1 and upvoted, then `a`
    clearing it to 0 and `z` flipping it to -1; `comment_count` + 1; the
    focus on the new comment with a reveal asked for; the `Comment posted`
    and `Reply posted` toasts.
  - Replying to a collapsed comment expanding it.
  - After a top-level insert, a later sibling and a descendant of one still
    collapsed (their shifted paths in `collapsed`), `n`/`p` walking the
    tree in order, and a vote landing on the comment meant; the same after
    a deep insert, with a comment in another branch untouched, and a
    collapsed comment expanded afterwards showing its reply, which takes a
    vote. `make_room` on its own shifting exactly the later siblings and
    their descendants, the focus included.
  - Replying on a post with no seeded comments, to your own comments five
    levels deep, a second top-level comment going before the first, and on
    a hidden post.
  - A submit dropping the last frame's click regions, so a click before the
    next frame does nothing.
  - Ctrl-W, Ctrl-U, Alt-x, Alt-q, Ctrl-Alt-a, Ctrl-Shift-W, Super-j,
    Ctrl/Alt-Backspace, Ctrl/Alt-Enter and Alt-Esc doing nothing in the
    composer (draft, mode, posts and thread unchanged), Shift-modified `A`,
    `?`, `Ñ` and Shift-Backspace working, Ctrl-C still quitting; search mode
    still typing Ctrl-W and Alt-x as `w` and `x`.
  - Opening the composer asking for a reveal, with `restore` set to the
    scroll; Esc returning to `restore` even when the composer's scroll
    moved, and dropping a pending reveal; a submit carrying the composer's
    scroll on and revealing the new comment.
- **`ui.rs`** (96), rendering into a `TestBackend`:
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
    focused, the composer for a comment and for the post, help) fitting in
    80 columns on every feed, Hidden included, in both the Card and Compact
    views, with the exact plain and group feed sets, `h hide` and `? help`
    in both, `J join` only in the feed set on group feeds, the exact
    search-mode, post-view (`r reply` in both), composer and help sets, the
    toggle's toast, a toast replacing the focused set, typing (`?`
    included) keeping the composer's set, `Esc` from the composer bringing
    back the post set it came from, and `Esc` bringing back the unfocused
    one. Closing the help bringing back the set it replaced.
  - The composer at 110×24 and 80×24, row by row: the ` Commenting on the
    post ` and ` Replying to u/bob ` titles, the cursor in orange right
    after the last char, `0/500` and `<n>/500` right-aligned on the bottom
    border, a long draft showing exactly the last 3 rows `wrap` makes, the
    post box ending right above it, and the composer's hints. `draft_rows`
    on its own: an empty draft, a trailing run shown as one space, a full
    row pushing the cursor down, non-ASCII chars, width 0.
  - At 24 rows and every width from 1 to 120, with the post scrolled: every
    cell outside the composer identical to the post view without it (at 19
    rows inside the post column, at 24 outside it), the composer's corners
    and sides on the column's edges, and the scroll, the scroll clamp and
    the comment and `Reply` regions those of the 19-row view, all above the
    composer. Tiny terminals (down to 1×1) not panicking.
  - A posted comment drawn first and focused, its trimmed body below, its
    `▲ 1 ▼` orange, then `▲ 0 ▼` after `a`, `··· 4 more comments` unchanged
    and the card back in the feed saying `▤ 11 Comments`. A reply to the
    last comment scrolled into view once, opening the composer keeping
    the last comment's header above it, the submit carrying that scroll on,
    and `k` free afterwards.
  - Collapsed `[+]` headers still drawn collapsed, their replies hidden,
    after a top-level insert and after a reply to an earlier comment.
  - `Reply` regions exactly on the drawn labels, 5 cells each, one per
    expanded comment; the blanks beside a label doing nothing; a click on
    its last cell focusing that comment and opening ` Replying to u/dan `
    (its header already above the composer, so nothing scrolls), and
    posting from there; a click on the focused
    comment's `Reply` opening the composer without collapsing; no `Reply`
    region in the feed. At every width from 8 to 60, scrolling through a
    deep thread, every region inside the viewport and the text area, its
    cells spelling the visible part of `Reply`, and every whole label drawn
    having a region.
  - With the composer open, left, right and middle clicks, releases, drags,
    moves and the wheel, on a header, a `Reply`, the composer, the search
    box and the nav: `handles_mouse` false and nothing changed, the draft
    included.
  - Opening the composer at 110×24 with dan's or fay's header at every
    scroll position: a header already above the composer not scrolling, one
    in the last 5 rows landing on the last row above it (the least scroll),
    one off screen brought into view, and Esc restoring the exact scroll and
    header row each time. With nothing focused, opening never scrolls and
    Esc restores the scroll. A click on dan's `Reply` on the last row of the
    post box scrolling by 3 so dan's header sits right above the composer,
    and Esc restoring the scroll with dan still focused.
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
  - The help overlay:
    - At 80×24, the box centered over the body (67×20 at column 6),
      titled ` Keys `, the header unchanged, the footer the help's set,
      every cell outside the box as without the help, and inside exactly the
      help's lines with blank padding: nothing shows through and nothing
      wraps. The feed's section first and `1-18 of N` on the bottom border.
    - The current mode's section first: Feed (with a kept search too), or
      Post view then Feed, the rest in table order (Search mode, Composer,
      Mouse, This help, Anywhere).
    - At 80×24, 100×30, 110×24 and 200×80, from the feed and the post view,
      scrolling through all of it: every line, and every description whole,
      on screen. Tall enough, no scrolling and no position shown.
    - Each scrolling key, clamped at both ends; `G` storing `u16::MAX` and
      the renderer clamping it so the last row (`Ctrl-C`) is the box's last;
      `G` then `k` moving at once; the feed never moving.
    - The wheel scrolling 3 lines from inside the box and from anywhere
      else, clamped at both ends, in the feed and the post view, the post's
      own scroll untouched.
    - A click outside closing it and changing nothing else (every field of
      `App` the same after a render) where the help-free frame drew a card,
      its `▲`, `▼` and `★ Saved`, a sort tab, the view switch, a nav entry,
      the search box and a comment header; also with no frame drawn since
      `?`. Clicks inside (corners, title, padding, text, over cards and
      comment headers) changing nothing.
    - With no frame drawn since `?`, so `hits` still holds the screen's
      regions and no box: a click on a card's `▲` and `▼`, the card, a sort
      tab, a nav entry, an unfocused comment header and the focused one
      (which a click would collapse), each first checked to be that region,
      closing the help and changing nothing else, before and after the next
      frame; the wheel not reaching the feed or the post.
    - Resizing while open: after `G` at 80×24, 80×50, 30×8, 80×80 and back
      at 80×24, the scroll re-clamped to the new content and box, the last
      line on the box's last row (the whole content, unscrolled, when it
      fits) and `k` moving on the very next frame; a scroll left at the old
      bottom clamped by a bigger box alone, and down to 1×1 no panic.
    - From cards and compact rows scrolled down, a group feed with a kept
      search, an empty feed and a post view with a focused and a collapsed
      comment, scrolled: `?`, `Esc` and `q` each closing it and restoring
      every field of `App`.
    - `a`, `z`, `s`, `h`, `J`, `v`, `c`, `n`, `p`, the digits, `[`, `]`,
      `/`, Enter, Tab, Backspace and other keys changing nothing, rendering
      after each, on a group feed, in the Compact view with a kept search
      and in the post view with a focused comment.
    - The mouse events the app ignores changing nothing and not counting,
      with the help open, inside and outside the box.
    - Every size from 1×1 to 32×12, plus 80×4, 80×5, 200×4, 30×24, 79×24
      and 63×22, from the feed and the post view: no panic, the box inside
      the body (or not drawn with no body), the header as without the help,
      the footer the help's set (where the body has 2 rows or more), and the
      scroll within its clamp after `G`.
    - Every key of every footer hint set listed in the help table under its
      mode's section (the composer's sets under Composer), along with the
      keys the feed sets leave out; each section once; and the footer only
      ever showing one of those sets, walking through the feed, search, the
      post view, the help and the composer for a reply and a comment.
    - Every char of the help table and the hint sets one cell wide in
      ratatui (`↓`, `↑`, `▲`, `▼` and `☆` included), and `▦`/`≡` unused.
- **`state.rs`** (28), on in-memory strings and on files in a directory
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
  - Ctrl-C with the help open saving the vote made before it and nothing
    pressed while it was open, the file unchanged in format, and the next
    run starting with the help closed.
  - A run that writes a top-level comment and votes on it, replies to a
    seeded reply and to that reply, then votes on a seeded reply and a
    seeded top-level comment that were both pushed down, saving a file byte
    for byte the one the same votes save without the comments; loading it
    restoring those votes onto the right comments, with the seed tree and
    seed comment count back; and the file written back the same.
  - Writing comments and nothing else creating no file.
  - Loading into an app that has your comments: records naming their keys
    matching nothing, your comments unchanged, a seeded record landing on
    the seeded comment at its shifted path.
  - A comment of yours with exactly a seeded sibling's key (a seed author
    called `you`, the same text) never saved and never taking the key: only
    the seeded comment's vote is written, and it loads onto the seeded
    comment, with or without yours in front of it.

Lint with `arc f`, `arc lint` and `arc rust-clippy`. The `arc lint -e extra`
engine isn't configured for `users/`.
