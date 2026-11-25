//! Votes, saves, hidden posts, read posts and group membership, kept between runs in a
//! small text file (see Persistence in PLAN.md).

use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process;

use crate::app;
use crate::app::App;
use crate::app::CommentPath;
use crate::data::Comment;
use crate::data::Vote;

/// The version line of a file with no `hidden` or `read` record. Builds from
/// before hiding existed read only this one, so it is written whenever it can
/// be.
const VERSION: &str = "workit-state 1";

/// The version line of a file with at least one `hidden` record and no `read`
/// record. A build from before hiding refuses it instead of dropping the
/// records it doesn't know, and one from before reading accepts it.
const VERSION_HIDDEN: &str = "workit-state 2";

/// The version line of a file with at least one `read` record. A build from
/// before reading refuses it instead of dropping the `read` records.
const VERSION_READ: &str = "workit-state 3";

/// The largest state file loaded. Far above anything the seed can produce:
/// every item in it changed is a few dozen KiB.
const MAX_BYTES: u64 = 1 << 20;

/// Fields in a post's key: its group's name, its author and its title.
const POST_KEY_LEN: usize = 3;

/// Where the state file lives: `$WORKIT_STATE` if non-empty, else
/// `$XDG_STATE_HOME/workit/state` if that is absolute, else
/// `$HOME/.local/state/workit/state` if that is absolute. `None` means run
/// without persistence.
pub(crate) fn path(
    workit_state: Option<&OsStr>,
    xdg_state_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Option<PathBuf> {
    if let Some(file) = workit_state.filter(|file| !file.is_empty()) {
        return Some(PathBuf::from(file));
    }
    if let Some(dir) = xdg_state_home
        .map(Path::new)
        .filter(|dir| dir.is_absolute())
    {
        return Some(dir.join("workit").join("state"));
    }
    home.map(Path::new)
        .filter(|home| home.is_absolute())
        .map(|home| home.join(".local/state/workit/state"))
}

/// `path` from the process environment.
pub(crate) fn path_from_env() -> Option<PathBuf> {
    path(
        env::var_os("WORKIT_STATE").as_deref(),
        env::var_os("XDG_STATE_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    )
}

/// Something whose state persists, by its position in the current data.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Group(usize),
    Post(usize),
    Comment(usize, CommentPath),
}

/// Every item with its key, in seed order: the groups, then each post
/// followed by its comments in pre-order. A group's key is its name, a
/// post's is its group's name, author and title, and a comment's is its
/// post's key followed by the author and body of each comment on the way
/// down to it, its own last.
///
/// Comments you wrote this session (`Comment::mine`) are not items, and
/// neither is anything under one: they are never saved, can't own a key,
/// and no record can name them. Each item's path is its current one, so a
/// seeded comment pushed to a later index by one of yours keeps its key.
fn items(app: &App) -> Vec<(Vec<&str>, Item)> {
    fn walk<'a>(
        comments: &'a [Comment],
        key: &[&'a str],
        path: &[usize],
        post: usize,
        out: &mut Vec<(Vec<&'a str>, Item)>,
    ) {
        for (n, comment) in comments.iter().enumerate() {
            if comment.mine {
                continue;
            }
            let key = [key, &[comment.author, &comment.body]].concat();
            let path = [path, &[n]].concat();
            out.push((key.clone(), Item::Comment(post, path.clone())));
            walk(&comment.replies, &key, &path, post, out);
        }
    }
    let mut out: Vec<_> = app
        .groups
        .iter()
        .enumerate()
        .map(|(g, group)| (vec![group.name], Item::Group(g)))
        .collect();
    for (i, post) in app.posts.iter().enumerate() {
        let key = vec![app.groups[post.group].name, post.author, post.title];
        out.push((key.clone(), Item::Post(i)));
        walk(&post.comments, &key, &[], i, &mut out);
    }
    out
}

/// Which item each key names. When several items share a key, the first in
/// seed order owns it; the others are never saved or loaded. Group keys have
/// one field and post and comment keys at least three, so they never clash.
fn owners<'a, 'k>(items: &'a [(Vec<&'k str>, Item)]) -> HashMap<&'a [&'k str], &'a Item> {
    let mut owners = HashMap::new();
    for (key, item) in items {
        owners.entry(key.as_slice()).or_insert(item);
    }
    owners
}

/// Backslash, tab, newline and carriage return become `\\`, `\t`, `\n` and
/// `\r`, so a field never holds the tab that separates fields or the newline
/// that ends a record. Everything else is written as is.
fn escape(field: &str, out: &mut String) {
    for ch in field.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
}

/// The inverse of `escape`. `None` for any other escape or a trailing `\`.
fn unescape(field: &str) -> Option<String> {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(ch) = chars.next() {
        out.push(match ch {
            '\\' => match chars.next()? {
                '\\' => '\\',
                't' => '\t',
                'n' => '\n',
                'r' => '\r',
                _ => return None,
            },
            _ => ch,
        });
    }
    Some(out)
}

fn push_record<'a>(out: &mut String, fields: impl IntoIterator<Item = &'a str>) {
    for (n, field) in fields.into_iter().enumerate() {
        if n > 0 {
            out.push('\t');
        }
        escape(field, out);
    }
    out.push('\n');
}

/// The state file for `app`: the version line, then one record for each
/// group whose `joined` differs from `seed_joined`, each saved post, each
/// hidden post, each read post, and each post or comment with a vote, in
/// seed order. The version is `VERSION_READ` when a `read` record is written,
/// else `VERSION_HIDDEN` when a `hidden` record is, and `VERSION` otherwise:
/// the oldest version that knows every record written.
fn encode(app: &App, seed_joined: &[bool]) -> String {
    let items = items(app);
    let owners = owners(&items);
    let mut out = String::new();
    let mut any_hidden = false;
    let mut any_read = false;
    for (key, item) in &items {
        if owners[key.as_slice()] != item {
            continue;
        }
        let mut record = |kind: &'static str, value: Option<&'static str>| {
            push_record(
                &mut out,
                [kind].into_iter().chain(value).chain(key.iter().copied()),
            );
        };
        let vote = match item {
            Item::Group(g) => {
                let joined = app.groups[*g].joined;
                if seed_joined.get(*g) != Some(&joined) {
                    record("group", Some(if joined { "joined" } else { "left" }));
                }
                continue;
            }
            Item::Post(i) => {
                let post = &app.posts[*i];
                if post.saved {
                    record("saved", None);
                }
                if post.hidden {
                    record("hidden", None);
                    any_hidden = true;
                }
                if post.read {
                    record("read", None);
                    any_read = true;
                }
                post.vote
            }
            Item::Comment(i, path) => {
                let comments = &app.posts[*i].comments;
                path[1..]
                    .iter()
                    .fold(&comments[path[0]], |comment, &n| &comment.replies[n])
                    .vote
            }
        };
        match vote {
            Vote::Up => record("vote", Some("up")),
            Vote::Down => record("vote", Some("down")),
            Vote::None => {}
        }
    }
    let version = if any_read {
        VERSION_READ
    } else if any_hidden {
        VERSION_HIDDEN
    } else {
        VERSION
    };
    format!("{version}\n{out}")
}

/// One line of a state file, unescaped but not yet matched to an item.
enum Record {
    Group(bool, String),
    Saved(Vec<String>),
    Hidden(Vec<String>),
    Read(Vec<String>),
    Vote(Vote, Vec<String>),
}

/// The record kinds a file's version line allows beyond version 1's.
#[derive(Debug, Clone, Copy)]
struct Known {
    /// `hidden`, in `VERSION_HIDDEN` and `VERSION_READ` files.
    hidden: bool,
    /// `read`, in `VERSION_READ` files only.
    read: bool,
}

impl Record {
    /// `None` for a malformed line: a bad escape, an unknown kind or value,
    /// or the wrong number of fields. `known` says which kinds this file's
    /// version allows: a `hidden` or `read` line in a version that doesn't
    /// know it is malformed.
    fn parse(line: &str, known: Known) -> Option<Record> {
        let fields: Vec<String> = line.split('\t').map(unescape).collect::<Option<_>>()?;
        let (kind, rest) = fields.split_first()?;
        match (kind.as_str(), rest) {
            ("group", [value, name]) => {
                let joined = match value.as_str() {
                    "joined" => true,
                    "left" => false,
                    _ => return None,
                };
                Some(Record::Group(joined, name.clone()))
            }
            ("saved", key) if key.len() == POST_KEY_LEN => Some(Record::Saved(key.to_vec())),
            ("hidden", key) if known.hidden && key.len() == POST_KEY_LEN => {
                Some(Record::Hidden(key.to_vec()))
            }
            ("read", key) if known.read && key.len() == POST_KEY_LEN => {
                Some(Record::Read(key.to_vec()))
            }
            ("vote", [value, key @ ..])
                if key.len() >= POST_KEY_LEN && (key.len() - POST_KEY_LEN).is_multiple_of(2) =>
            {
                let vote = match value.as_str() {
                    "up" => Vote::Up,
                    "down" => Vote::Down,
                    _ => return None,
                };
                Some(Record::Vote(vote, key.to_vec()))
            }
            _ => None,
        }
    }

    /// What the record asks of the item its key names, or `None` when the
    /// key names nothing in the current data.
    fn resolve(self, owners: &HashMap<&[&str], &Item>) -> Option<Change> {
        let owner = |key: &[String]| {
            let key: Vec<&str> = key.iter().map(String::as_str).collect();
            owners.get(key.as_slice()).copied()
        };
        match self {
            Record::Group(joined, name) => match owner(&[name])? {
                Item::Group(g) => Some(Change::Join(*g, joined)),
                _ => None,
            },
            Record::Saved(key) => match owner(&key)? {
                Item::Post(i) => Some(Change::Save(*i)),
                _ => None,
            },
            Record::Hidden(key) => match owner(&key)? {
                Item::Post(i) => Some(Change::Hide(*i)),
                _ => None,
            },
            Record::Read(key) => match owner(&key)? {
                Item::Post(i) => Some(Change::Read(*i)),
                _ => None,
            },
            Record::Vote(vote, key) => match owner(&key)? {
                Item::Post(i) => Some(Change::PostVote(*i, vote)),
                Item::Comment(i, path) => Some(Change::CommentVote(*i, path.clone(), vote)),
                Item::Group(_) => None,
            },
        }
    }
}

enum Change {
    Join(usize, bool),
    Save(usize),
    Hide(usize),
    Read(usize),
    PostVote(usize, Vote),
    CommentVote(usize, CommentPath, Vote),
}

/// Sets a vote that is still the seed's (`Vote::None`) with `cast`'s
/// arithmetic. A record repeated in the file doesn't clear it again.
fn set_vote(current: &mut Vote, score: &mut i32, vote: Vote) {
    if *current != vote {
        app::cast(current, score, vote);
    }
}

/// The file's first line isn't `VERSION`, `VERSION_HIDDEN` or `VERSION_READ`:
/// it is empty, garbage, binary, or from a newer version.
#[derive(Debug, PartialEq, Eq)]
struct Unrecognized;

/// Applies the state file `bytes` to `app`, fresh from the seed, as if the
/// changes had been made by hand, then rebuilds the feed. Malformed lines
/// are skipped, and their count returned; records whose key names nothing
/// are dropped.
fn apply(app: &mut App, bytes: &[u8]) -> Result<usize, Unrecognized> {
    let mut lines = bytes.split(|&b| b == b'\n');
    let known = match lines.next() {
        Some(line) if line == VERSION.as_bytes() => Known {
            hidden: false,
            read: false,
        },
        Some(line) if line == VERSION_HIDDEN.as_bytes() => Known {
            hidden: true,
            read: false,
        },
        Some(line) if line == VERSION_READ.as_bytes() => Known {
            hidden: true,
            read: true,
        },
        _ => return Err(Unrecognized),
    };
    let mut skipped = 0;
    let changes: Vec<Change> = {
        let items = items(app);
        let owners = owners(&items);
        lines
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let record = std::str::from_utf8(line)
                    .ok()
                    .and_then(|line| Record::parse(line, known));
                if record.is_none() {
                    skipped += 1;
                }
                record?.resolve(&owners)
            })
            .collect()
    };
    for change in changes {
        match change {
            Change::Join(g, joined) => app.groups[g].joined = joined,
            Change::Save(i) => app.posts[i].saved = true,
            Change::Hide(i) => app.posts[i].hidden = true,
            Change::Read(i) => app.posts[i].read = true,
            Change::PostVote(i, vote) => {
                let post = &mut app.posts[i];
                set_vote(&mut post.vote, &mut post.score, vote);
            }
            Change::CommentVote(i, path, vote) => {
                if let Some(comment) = app::comment_mut(&mut app.posts[i].comments, &path) {
                    set_vote(&mut comment.vote, &mut comment.score, vote);
                }
            }
        }
    }
    app.refresh();
    Ok(skipped)
}

/// Reads the state file at `path`: only a regular file (after following
/// symlinks) of at most `MAX_BYTES`, so a FIFO, a device or a huge file can't
/// hang the app or exhaust memory before the first frame. The metadata is
/// checked before opening, so a FIFO is never opened, and again on the opened
/// file. A missing file is a `NotFound` error.
fn read(path: &Path) -> io::Result<Vec<u8>> {
    let too_large = || io::Error::other(format!("larger than {} MiB", MAX_BYTES >> 20));
    let check = |meta: fs::Metadata| {
        if !meta.is_file() {
            Err(io::Error::other("not a regular file"))
        } else if meta.len() > MAX_BYTES {
            Err(too_large())
        } else {
            Ok(())
        }
    };
    check(fs::metadata(path)?)?;
    let file = fs::File::open(path)?;
    check(file.metadata()?)?;
    // In case it grows after the check.
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

/// Writes `bytes` to a temp file next to `path`, then renames it over
/// `path`, so `path` always holds either the old contents or the new ones.
/// The temp file gets the permissions of the file it replaces, if any, before
/// anything is written to it. Creates missing parent directories.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the path has no file name"))?;
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    fs::create_dir_all(dir)?;
    let mut temp = OsString::from(".");
    temp.push(name);
    temp.push(format!(".{}.tmp", process::id()));
    let temp = dir.join(temp);
    let written = fs::File::create(&temp)
        .and_then(|mut file| {
            if let Ok(old) = fs::metadata(path)
                && old.is_file()
            {
                file.set_permissions(old.permissions())?;
            }
            file.write_all(bytes)?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temp, path));
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    } else if let Ok(dir) = fs::File::open(dir) {
        // Best effort: makes the rename itself durable.
        let _ = dir.sync_all();
    }
    written
}

/// The state file of one run: loaded once before the first frame, saved
/// once on a normal quit.
pub(crate) struct Store {
    path: PathBuf,
    /// Each group's `joined` in the seed, before the file was applied.
    seed_joined: Vec<bool>,
    /// False when the file couldn't be read or has an unknown version. It
    /// is then left exactly as it is.
    writable: bool,
}

impl Store {
    /// Loads the file at `path` into `app`, which must be fresh from the
    /// seed. Never fails: a missing file means the seed, silently, and any
    /// other problem comes back as a message to print once the terminal is
    /// restored.
    pub(crate) fn load(path: PathBuf, app: &mut App) -> (Store, Option<String>) {
        let mut store = Store {
            seed_joined: app.groups.iter().map(|group| group.joined).collect(),
            path,
            writable: true,
        };
        let shown = store.path.display();
        let message = match read(&store.path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => {
                store.writable = false;
                Some(format!(
                    "couldn't read {shown}: {e}. This run's changes weren't saved."
                ))
            }
            Ok(bytes) => match apply(app, &bytes) {
                Ok(0) => None,
                Ok(skipped) => Some(format!(
                    "ignored {skipped} malformed line(s) in {shown}. Saving drops them."
                )),
                Err(Unrecognized) => {
                    store.writable = false;
                    Some(format!(
                        "{shown} doesn't start with \"{VERSION}\", \"{VERSION_HIDDEN}\" or \
                         \"{VERSION_READ}\", so it wasn't loaded and was left unchanged. This run's \
                         changes weren't saved."
                    ))
                }
            },
        };
        (store, message)
    }

    /// Writes `app`'s differences from the seed. Does nothing when the file
    /// is to be left alone, or when there is nothing to store and no file.
    pub(crate) fn save(&self, app: &App) -> Result<(), String> {
        if !self.writable {
            return Ok(());
        }
        let text = encode(app, &self.seed_joined);
        let shown = self.path.display();
        let fail = |e: io::Error| format!("couldn't save {shown}: {e}");
        let entries = text.lines().count() - 1;
        if entries == 0 && !self.path.try_exists().map_err(fail)? {
            return Ok(());
        }
        write_atomic(&self.path, text.as_bytes()).map_err(fail)
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;
    use ratatui::crossterm::event::KeyEvent;
    use ratatui::crossterm::event::KeyModifiers;

    use super::*;
    use crate::app::Feed;
    use crate::data;
    use crate::data::Group;
    use crate::data::Post;
    use crate::data::c;

    fn press(app: &mut App, keys: &str) {
        for ch in keys.chars() {
            let code = if ch == '\n' {
                KeyCode::Enter
            } else {
                KeyCode::Char(ch)
            };
            app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
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

    fn post(
        groups: &[Group],
        group: &str,
        author: &'static str,
        title: &'static str,
        score: i32,
        hours_ago: u32,
        comments: Vec<Comment>,
    ) -> Post {
        Post {
            group: groups.iter().position(|g| g.name == group).unwrap(),
            author,
            title,
            score,
            hours_ago,
            comments,
            ..Default::default()
        }
    }

    /// Three groups and three posts. The first post's comments, in display
    /// order: marco, dana, lin (depth 2), sam.
    fn seed() -> App {
        let groups = vec![
            group("Engineering", true),
            group("Dogs", false),
            group("Food", true),
        ];
        let posts = vec![
            post(
                &groups,
                "Engineering",
                "dana",
                "dead code",
                100,
                1,
                vec![
                    c("marco", 10, 1, "congrats").with(vec![
                        c("dana", 20, 1, "thanks").with(vec![c("lin", 30, 1, "deep")]),
                    ]),
                    c("sam", 40, 1, "broke?"),
                ],
            ),
            post(&groups, "Dogs", "kim", "biscuit", 50, 2, vec![]),
            post(&groups, "Food", "chef", "pizza", 70, 3, vec![]),
        ];
        App::with_data(groups, posts)
    }

    /// The same items as `seed`, reordered, plus a new group, a new post and
    /// new sibling comments at depths 0 and 2.
    fn reordered_seed() -> App {
        let groups = vec![
            group("Food", true),
            group("Hackathon", false),
            group("Dogs", false),
            group("Engineering", true),
        ];
        let posts = vec![
            post(&groups, "Hackathon", "ravi", "new post", 5, 1, vec![]),
            post(&groups, "Food", "chef", "pizza", 70, 3, vec![]),
            post(
                &groups,
                "Engineering",
                "dana",
                "dead code",
                100,
                1,
                vec![
                    c("new", 1, 1, "first!"),
                    c("sam", 40, 1, "broke?"),
                    c("marco", 10, 1, "congrats").with(vec![
                        c("dana", 20, 1, "thanks")
                            .with(vec![c("new", 2, 1, "me too"), c("lin", 30, 1, "deep")]),
                    ]),
                ],
            ),
            post(&groups, "Dogs", "kim", "biscuit", 50, 2, vec![]),
        ];
        App::with_data(groups, posts)
    }

    fn titles(app: &App) -> Vec<&'static str> {
        app.view.iter().map(|&i| app.posts[i].title).collect()
    }

    fn find<'a>(app: &'a App, title: &str) -> &'a Post {
        app.posts.iter().find(|p| p.title == title).unwrap()
    }

    fn joined(app: &App, name: &str) -> bool {
        app.groups.iter().find(|g| g.name == name).unwrap().joined
    }

    fn seed_joined(app: &App) -> Vec<bool> {
        app.groups.iter().map(|g| g.joined).collect()
    }

    /// Upvotes "dead code", downvotes and saves "pizza", downvotes lin
    /// (depth 2) and upvotes sam, leaves Engineering and joins Dogs, all by
    /// key presses. Opening "dead code" to vote on its comments marks it
    /// read. Ends back on Home.
    fn change_everything(app: &mut App) {
        assert_eq!(titles(app), ["dead code", "pizza"]);
        press(app, "ajzs");
        press(app, "g\nnnnznaq");
        press(app, "]]]]]J]J");
        assert_eq!(app.feed, Feed::Group(1));
        press(app, "[[[[[[");
        assert_eq!(app.feed, Feed::Home);
    }

    /// Undoes `change_everything` after a restore, by key presses. Opening
    /// "dead code" again leaves it read, so `m` marks it unread.
    fn undo_everything(app: &mut App) {
        assert_eq!(titles(app), ["pizza", "biscuit"]);
        press(app, "zs]]");
        assert_eq!(titles(app), ["dead code", "pizza", "biscuit"]);
        press(app, "a\nnnnznamq");
        assert!(!find(app, "dead code").read);
        press(app, "]]]J]J");
        assert_eq!(app.feed, Feed::Group(1));
    }

    const CHANGED: &str = "workit-state 1\n\
        group\tleft\tEngineering\n\
        group\tjoined\tDogs\n\
        vote\tup\tEngineering\tdana\tdead code\n\
        vote\tdown\tEngineering\tdana\tdead code\tmarco\tcongrats\tdana\tthanks\tlin\tdeep\n\
        vote\tup\tEngineering\tdana\tdead code\tsam\tbroke?\n\
        saved\tFood\tchef\tpizza\n\
        vote\tdown\tFood\tchef\tpizza\n";

    /// What `change_everything` saves: `CHANGED` with "dead code" read, so
    /// version 3, its `read` record before its vote.
    const CHANGED_READ: &str = "workit-state 3\n\
        group\tleft\tEngineering\n\
        group\tjoined\tDogs\n\
        read\tEngineering\tdana\tdead code\n\
        vote\tup\tEngineering\tdana\tdead code\n\
        vote\tdown\tEngineering\tdana\tdead code\tmarco\tcongrats\tdana\tthanks\tlin\tdeep\n\
        vote\tup\tEngineering\tdana\tdead code\tsam\tbroke?\n\
        saved\tFood\tchef\tpizza\n\
        vote\tdown\tFood\tchef\tpizza\n";

    /// A directory under the system temp dir, removed when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            let dir = env::temp_dir().join(format!("workit-test-{}-{name}", process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn entries(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(&self.0)
                .unwrap()
                .map(|e| e.unwrap().file_name().into_string().unwrap())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// One run: loads `path` into `app`, makes `changes`, quits and saves.
    fn run(path: &Path, mut app: App, changes: impl FnOnce(&mut App)) -> App {
        let (store, message) = Store::load(path.to_owned(), &mut app);
        assert_eq!(message, None);
        changes(&mut app);
        store.save(&app).unwrap();
        app
    }

    #[test]
    fn round_trip_restores_votes_saves_and_membership() {
        let mut app = seed();
        change_everything(&mut app);
        let text = encode(&app, &seed_joined(&seed()));
        assert_eq!(text, CHANGED_READ, "seed order, only what differs");

        let mut app = seed();
        assert_eq!(apply(&mut app, text.as_bytes()), Ok(0));
        let dead = find(&app, "dead code");
        assert_eq!(
            (dead.score, dead.vote, dead.read),
            (101, Vote::Up, true),
            "seed + 1, not + 2"
        );
        assert!(!find(&app, "pizza").read && !find(&app, "biscuit").read);
        let pizza = find(&app, "pizza");
        assert_eq!(
            (pizza.score, pizza.vote, pizza.saved),
            (69, Vote::Down, true)
        );
        let biscuit = find(&app, "biscuit");
        assert_eq!(
            (biscuit.score, biscuit.vote, biscuit.saved),
            (50, Vote::None, false)
        );
        let marco = &dead.comments[0];
        let lin = &marco.replies[0].replies[0];
        assert_eq!((lin.score, lin.vote), (29, Vote::Down));
        assert_eq!(
            (dead.comments[1].score, dead.comments[1].vote),
            (41, Vote::Up)
        );
        assert_eq!((marco.score, marco.vote), (10, Vote::None));
        assert!(!joined(&app, "Engineering") && joined(&app, "Dogs") && joined(&app, "Food"));

        assert_eq!((app.feed, app.selected), (Feed::Home, 0));
        assert_eq!(
            titles(&app),
            ["pizza", "biscuit"],
            "Home's first build has the restored membership"
        );
        press(&mut app, "]]]");
        assert_eq!(app.feed, Feed::Saved);
        assert_eq!(titles(&app), ["pizza"], "and Saved the restored save");
        assert_eq!(encode(&app, &seed_joined(&seed())), CHANGED_READ);
    }

    #[test]
    fn ctrl_c_with_the_help_open_saves_and_the_help_is_never_saved() {
        let dir = TempDir::new("help-ctrl-c");
        let path = dir.0.join("state");
        run(&path, seed(), |app| {
            press(app, "a?Gjz");
            assert!(app.help.is_some());
            app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
            assert!(app.should_quit);
        });
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "workit-state 1\nvote\tup\tEngineering\tdana\tdead code\n",
            "the vote before the help, and nothing pressed while it was open"
        );
        let app = run(&path, seed(), |_| {});
        assert_eq!(app.help, None, "every run starts with the help closed");
        assert_eq!(find(&app, "dead code").score, 101);
    }

    #[test]
    fn restored_upvote_undone_with_a_gives_the_seed_score() {
        let dir = TempDir::new("undo-upvote");
        let path = dir.0.join("state");
        run(&path, seed(), |app| press(app, "a"));
        let app = run(&path, seed(), |app| {
            assert_eq!(find(app, "dead code").score, 101);
            press(app, "a");
        });
        let dead = find(&app, "dead code");
        assert_eq!((dead.score, dead.vote), (100, Vote::None));
        assert_eq!(fs::read_to_string(&path).unwrap(), "workit-state 1\n");
    }

    #[test]
    fn undoing_every_change_leaves_a_file_with_no_entries() {
        let dir = TempDir::new("undo-all");
        let path = dir.0.join("state");
        run(&path, seed(), change_everything);
        assert_eq!(fs::read_to_string(&path).unwrap(), CHANGED_READ);
        run(&path, seed(), undo_everything);
        assert_eq!(fs::read_to_string(&path).unwrap(), "workit-state 1\n");
        let app = run(&path, seed(), |_| {});
        assert_eq!(encode(&app, &seed_joined(&seed())), "workit-state 1\n");
        assert_eq!(dir.entries(), ["state"]);
    }

    #[test]
    fn nothing_changed_and_no_file_creates_nothing() {
        let dir = TempDir::new("no-file");
        let path = dir.0.join("sub").join("state");
        run(&path, seed(), |app| press(app, "aa]]]]]JJ"));
        assert!(!dir.0.join("sub").exists(), "not even the directory");
    }

    #[test]
    fn keys_survive_a_reordered_and_extended_seed() {
        let mut app = seed();
        change_everything(&mut app);
        let text = encode(&app, &seed_joined(&seed()));

        let mut app = reordered_seed();
        let seed_joined = seed_joined(&app);
        assert_eq!(apply(&mut app, text.as_bytes()), Ok(0));
        let dead = find(&app, "dead code");
        assert_eq!((dead.score, dead.vote, dead.read), (101, Vote::Up, true));
        let pizza = find(&app, "pizza");
        assert_eq!(
            (pizza.score, pizza.vote, pizza.saved, pizza.read),
            (69, Vote::Down, true, false)
        );
        let [new, sam, marco] = &dead.comments[..] else {
            panic!("three top-level comments");
        };
        assert_eq!((new.author, new.score, new.vote), ("new", 1, Vote::None));
        assert_eq!((sam.author, sam.score, sam.vote), ("sam", 41, Vote::Up));
        assert_eq!((marco.score, marco.vote), (10, Vote::None));
        let [me_too, lin] = &marco.replies[0].replies[..] else {
            panic!("two replies at depth 2");
        };
        assert_eq!((me_too.score, me_too.vote), (2, Vote::None));
        assert_eq!((lin.author, lin.score, lin.vote), ("lin", 29, Vote::Down));
        let new_post = find(&app, "new post");
        assert_eq!(
            (new_post.score, new_post.vote, new_post.saved, new_post.read),
            (5, Vote::None, false, false)
        );
        assert!(!joined(&app, "Engineering") && joined(&app, "Dogs"));
        assert!(joined(&app, "Food") && !joined(&app, "Hackathon"));

        // Saved again, the records come out in the new seed order.
        assert_eq!(
            encode(&app, &seed_joined),
            "workit-state 3\n\
             group\tjoined\tDogs\n\
             group\tleft\tEngineering\n\
             saved\tFood\tchef\tpizza\n\
             vote\tdown\tFood\tchef\tpizza\n\
             read\tEngineering\tdana\tdead code\n\
             vote\tup\tEngineering\tdana\tdead code\n\
             vote\tup\tEngineering\tdana\tdead code\tsam\tbroke?\n\
             vote\tdown\tEngineering\tdana\tdead code\tmarco\tcongrats\tdana\tthanks\tlin\tdeep\n"
        );
    }

    #[test]
    fn keys_that_match_nothing_are_dropped() {
        let text = "workit-state 1\n\
            group\tjoined\tGone\n\
            group\tjoined\tdana\n\
            saved\tFood\tchef\tpasta\n\
            vote\tup\tFood\tsomeone\tpizza\n\
            vote\tup\tEngineering\tdana\tdead code\tsam\tfixed?\n\
            vote\tup\tEngineering\tdana\tdead code\tlin\tdeep\n\
            vote\tdown\tFood\tchef\tpizza\n";
        let mut app = seed();
        assert_eq!(apply(&mut app, text.as_bytes()), Ok(0), "not malformed");
        assert_eq!(
            encode(&app, &seed_joined(&seed())),
            "workit-state 1\nvote\tdown\tFood\tchef\tpizza\n",
            "only the one record that matched is kept"
        );
    }

    #[test]
    fn keys_with_separators_and_non_ascii_round_trip() {
        let groups = vec![group("Eng\tTabs = 1", false)];
        let title = "a\tb\nc\\d = e\\t \\n jalapeño 64°F İ \r";
        let body = "line 1\nline 2\t\\ = ✔";
        let posts = vec![
            post(
                &groups,
                "Eng\tTabs = 1",
                "dana\\k",
                title,
                10,
                1,
                vec![c("ñ=", 5, 1, body)],
            ),
            // Its key differs only in the escaped characters.
            post(
                &groups,
                "Eng\tTabs = 1",
                "dana\\k",
                "a\\tb\\nc\\\\d = e\\\\t \\\\n jalapeño 64°F İ \\r",
                10,
                1,
                vec![],
            ),
        ];
        let mut app = App::with_data(groups.clone(), posts.clone());
        let seed_joined = seed_joined(&app);
        press(&mut app, "]]]]]J");
        app.posts[0].saved = true;
        let first = &mut app.posts[0];
        app::cast(&mut first.vote, &mut first.score, Vote::Down);
        let comment = &mut app.posts[0].comments[0];
        app::cast(&mut comment.vote, &mut comment.score, Vote::Up);
        let text = encode(&app, &seed_joined);
        assert_eq!(text.lines().count(), 5, "one record per line: {text:?}");
        assert!(text.lines().all(|line| !line.contains('\r')));

        let mut restored = App::with_data(groups, posts);
        assert_eq!(apply(&mut restored, text.as_bytes()), Ok(0));
        assert!(restored.groups[0].joined);
        let [first, second] = &restored.posts[..] else {
            panic!("two posts");
        };
        assert_eq!(
            (first.score, first.vote, first.saved),
            (9, Vote::Down, true)
        );
        assert_eq!(
            (first.comments[0].score, first.comments[0].vote),
            (6, Vote::Up)
        );
        assert_eq!(
            (second.score, second.vote, second.saved),
            (10, Vote::None, false)
        );

        for field in [title, body, "", "\\", "\\\\t", "=\t=\n="] {
            let mut escaped = String::new();
            escape(field, &mut escaped);
            assert!(!escaped.contains(['\t', '\n', '\r']));
            assert_eq!(unescape(&escaped).as_deref(), Some(field));
        }
    }

    #[test]
    fn malformed_lines_are_skipped_and_the_rest_loads() {
        let mut bytes = b"workit-state 1\n\
            group\tmaybe\tDogs\n\
            group\tjoined\n\
            group\tjoined\tDogs\textra\n\
            vote\tsideways\tEngineering\tdana\tdead code\n\
            vote\tnone\tEngineering\tdana\tdead code\n\
            vote\tup\tEngineering\tdana\n\
            vote\tup\tEngineering\tdana\tdead code\tsam\n\
            saved\tFood\tchef\n\
            saved\tFood\tchef\tpizza\textra\n\
            bookmark\tFood\tchef\tpizza\n\
            vote\tup\tEngineering\tdana\tdead\\x code\n\
            vote\tup\tEngineering\tdana\tdead code\\\n\
            \n\
            vote\tup\tEngineering\tdana\tdead code\n\
            vote\tup\tEngineering\tdana\tdead code\n\
            saved\tFood\tchef\tpizza\n"
            .to_vec();
        bytes.extend(b"group\tjoined\tDo\xffgs\n");
        bytes.extend(b"group\tjoined\tDogs");
        let mut app = seed();
        assert_eq!(apply(&mut app, &bytes), Ok(13));
        let dead = find(&app, "dead code");
        assert_eq!(
            (dead.score, dead.vote),
            (101, Vote::Up),
            "a repeated record doesn't clear the vote"
        );
        assert!(find(&app, "pizza").saved);
        assert!(joined(&app, "Dogs"), "the last line needs no newline");
        assert_eq!(find(&app, "biscuit").score, 50);
    }

    #[test]
    fn unknown_or_missing_version_loads_nothing_and_is_left_alone() {
        let dir = TempDir::new("version");
        let path = dir.0.join("state");
        for contents in [
            &b""[..],
            b"\n",
            b"workit-state 4\nvote\tup\tEngineering\tdana\tdead code\n",
            b"workit-state 3 \nvote\tup\tEngineering\tdana\tdead code\n",
            b"workit-state 2 \nvote\tup\tEngineering\tdana\tdead code\n",
            b"workit-state 1 \nvote\tup\tEngineering\tdana\tdead code\n",
            b"vote\tup\tEngineering\tdana\tdead code\n",
            b"\x00\x9f\x92\x96binary\n",
        ] {
            assert_eq!(apply(&mut seed(), contents), Err(Unrecognized));
            fs::write(&path, contents).unwrap();
            let mut app = seed();
            let (store, message) = Store::load(path.clone(), &mut app);
            assert!(message.unwrap().contains("left unchanged"));
            assert_eq!(find(&app, "dead code").score, 100, "{contents:?}");
            press(&mut app, "zs");
            store.save(&app).unwrap();
            assert_eq!(fs::read(&path).unwrap(), contents, "not overwritten");
        }
        assert_eq!(dir.entries(), ["state"]);
    }

    #[test]
    fn unreadable_file_is_reported_and_left_alone() {
        let dir = TempDir::new("unreadable");
        let path = dir.0.join("state");
        fs::create_dir(&path).unwrap();
        let mut app = seed();
        let (store, message) = Store::load(path.clone(), &mut app);
        assert!(message.unwrap().starts_with("couldn't read "));
        press(&mut app, "a");
        assert_eq!(store.save(&app), Ok(()), "nothing is written");
        assert!(path.is_dir());
        assert_eq!(dir.entries(), ["state"]);

        // A directory where the file goes is a save error too, when the load
        // didn't see it.
        let store = Store {
            path: path.clone(),
            seed_joined: seed_joined(&seed()),
            writable: true,
        };
        assert!(store.save(&app).unwrap_err().starts_with("couldn't save "));
        assert_eq!(dir.entries(), ["state"], "no temp file left behind");
    }

    /// Loads `path` into a fresh seed, votes and saves: nothing loads, the
    /// message says the file couldn't be read, and nothing is written.
    fn assert_refused(dir: &TempDir, path: &Path, why: &str) {
        let mut app = seed();
        let (store, message) = Store::load(path.to_owned(), &mut app);
        let message = message.unwrap();
        assert!(
            message.starts_with("couldn't read ") && message.contains(why),
            "{message}"
        );
        assert_eq!(find(&app, "dead code").score, 100, "nothing loaded");
        press(&mut app, "as");
        assert_eq!(store.save(&app), Ok(()));
        assert!(!dir.entries().iter().any(|name| name.ends_with(".tmp")));
    }

    #[cfg(unix)]
    #[test]
    fn non_regular_files_are_refused_without_blocking() {
        let dir = TempDir::new("non-regular");
        let sub = dir.0.join("sub");
        fs::create_dir(&sub).unwrap();
        assert_refused(&dir, &sub, "not a regular file");
        assert!(sub.is_dir() && fs::read_dir(&sub).unwrap().next().is_none());
        assert_refused(&dir, Path::new("/dev/zero"), "not a regular file");

        let fifo = dir.0.join("fifo");
        match process::Command::new("mkfifo").arg(&fifo).status() {
            Ok(status) if status.success() => {
                // Opening a FIFO with no writer would block forever.
                assert_refused(&dir, &fifo, "not a regular file");
                let link = dir.0.join("link");
                std::os::unix::fs::symlink(&fifo, &link).unwrap();
                assert_refused(&dir, &link, "not a regular file");
                use std::os::unix::fs::FileTypeExt;
                assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
                assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
            }
            _ => eprintln!("mkfifo is unavailable; skipping the FIFO case"),
        }
    }

    #[test]
    fn oversized_files_are_refused_and_left_alone() {
        let dir = TempDir::new("oversized");
        let path = dir.0.join("state");
        let record = "vote\tup\tEngineering\tdana\tdead code\n";
        let mut text = format!("{VERSION}\n{record}");
        text.push_str(&"\n".repeat(MAX_BYTES as usize - text.len()));
        fs::write(&path, &text).unwrap();
        let mut app = seed();
        let (_, message) = Store::load(path.clone(), &mut app);
        assert_eq!(message, None, "exactly the cap loads");
        assert_eq!(find(&app, "dead code").score, 101);

        text.push('\n');
        fs::write(&path, &text).unwrap();
        assert_refused(&dir, &path, "larger than 1 MiB");
        assert_eq!(fs::read_to_string(&path).unwrap(), text, "not overwritten");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_to_a_regular_file_loads() {
        let dir = TempDir::new("symlink");
        let target = dir.0.join("target");
        fs::write(
            &target,
            "workit-state 1\nvote\tup\tEngineering\tdana\tdead code\n",
        )
        .unwrap();
        let link = dir.0.join("state");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let mut app = seed();
        assert_eq!(Store::load(link, &mut app).1, None);
        assert_eq!(find(&app, "dead code").score, 101);
    }

    #[cfg(unix)]
    #[test]
    fn save_keeps_the_old_files_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        let dir = TempDir::new("permissions");
        let path = dir.0.join("state");
        run(&path, seed(), |app| press(app, "a"));
        let default = dir.0.join("default");
        fs::File::create(&default).unwrap();
        assert_eq!(mode(&path), mode(&default), "a new file gets the default");

        for kept in [0o600, 0o640, 0o400] {
            fs::set_permissions(&path, fs::Permissions::from_mode(kept)).unwrap();
            run(&path, seed(), |app| press(app, "a"));
            assert_eq!(mode(&path), kept, "{kept:o}");
            run(&path, seed(), |app| press(app, "a"));
            assert_eq!(mode(&path), kept, "{kept:o}");
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                "workit-state 1\nvote\tup\tEngineering\tdana\tdead code\n"
            );
        }
        assert_eq!(dir.entries(), ["default", "state"]);
    }

    #[test]
    fn path_comes_from_workit_state_then_xdg_then_home() {
        let os = |s: &'static str| Some(OsStr::new(s));
        assert_eq!(
            path(os("/tmp/w"), os("/xdg"), os("/home/me")),
            Some(PathBuf::from("/tmp/w"))
        );
        assert_eq!(
            path(os("rel/w"), None, None),
            Some(PathBuf::from("rel/w")),
            "WORKIT_STATE is used as given"
        );
        assert_eq!(
            path(os(""), os("/xdg"), os("/home/me")),
            Some(PathBuf::from("/xdg/workit/state")),
            "an empty WORKIT_STATE is ignored"
        );
        assert_eq!(
            path(None, os("relative/xdg"), os("/home/me")),
            Some(PathBuf::from("/home/me/.local/state/workit/state")),
            "a relative XDG_STATE_HOME is ignored"
        );
        assert_eq!(
            path(None, os(""), os("/home/me")),
            Some(PathBuf::from("/home/me/.local/state/workit/state"))
        );
        assert_eq!(
            path(None, None, os("/home/me")),
            Some(PathBuf::from("/home/me/.local/state/workit/state"))
        );
        assert_eq!(path(os(""), os("rel"), os("")), None);
        assert_eq!(path(None, None, os("relative")), None);
        assert_eq!(path(None, None, None), None);
    }

    #[test]
    fn save_creates_missing_directories_and_leaves_no_temp_file() {
        let dir = TempDir::new("mkdir");
        let path = dir.0.join("a").join("b").join("state");
        run(&path, seed(), |app| press(app, "a"));
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "workit-state 1\nvote\tup\tEngineering\tdana\tdead code\n"
        );
        run(&path, seed(), |app| press(app, "js"));
        assert_eq!(
            fs::read_dir(path.parent().unwrap())
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect::<Vec<_>>(),
            ["state"]
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "workit-state 1\n\
             vote\tup\tEngineering\tdana\tdead code\n\
             saved\tFood\tchef\tpizza\n"
        );
    }

    #[test]
    fn a_shared_key_belongs_to_the_first_item() {
        let groups = vec![group("Food", true)];
        let posts = vec![
            post(
                &groups,
                "Food",
                "chef",
                "pizza",
                10,
                1,
                vec![c("x", 1, 1, "same"), c("x", 1, 1, "same")],
            ),
            post(&groups, "Food", "chef", "pizza", 20, 2, vec![]),
        ];
        let mut app = App::with_data(groups.clone(), posts.clone());
        for post in &mut app.posts {
            post.saved = true;
            app::cast(&mut post.vote, &mut post.score, Vote::Up);
        }
        let second = &mut app.posts[0].comments[1];
        app::cast(&mut second.vote, &mut second.score, Vote::Down);
        let text = encode(&app, &[true]);
        assert_eq!(
            text, "workit-state 1\nsaved\tFood\tchef\tpizza\nvote\tup\tFood\tchef\tpizza\n",
            "the second post and the second comment aren't saved"
        );

        let mut app = App::with_data(groups, posts);
        let doubled = format!("{text}vote\tdown\tFood\tchef\tpizza\tx\tsame\n");
        assert_eq!(apply(&mut app, doubled.as_bytes()), Ok(0));
        assert_eq!((app.posts[0].score, app.posts[0].saved), (11, true));
        assert_eq!((app.posts[1].score, app.posts[1].saved), (20, false));
        assert_eq!(app.posts[0].comments[0].vote, Vote::Down);
        assert_eq!(app.posts[0].comments[1].vote, Vote::None);
    }

    #[test]
    fn seed_keys_are_unique_and_the_seed_has_no_votes_or_saves() {
        let app = App::with_data(data::groups(), data::posts());
        let items = items(&app);
        assert_eq!(
            owners(&items).len(),
            items.len(),
            "every seed key is unique"
        );
        assert_eq!(encode(&app, &seed_joined(&app)), "workit-state 1\n");
        assert!(
            app.posts.iter().all(|p| !p.read),
            "every post starts unread"
        );
    }

    /// Downvotes "dead code", then upvotes, saves and hides "pizza", by key
    /// presses on Home.
    const HIDDEN: &str = "workit-state 2\n\
        vote\tdown\tEngineering\tdana\tdead code\n\
        saved\tFood\tchef\tpizza\n\
        hidden\tFood\tchef\tpizza\n\
        vote\tup\tFood\tchef\tpizza\n";

    #[test]
    fn a_hidden_post_round_trips_with_version_2() {
        let dir = TempDir::new("hidden");
        let path = dir.0.join("state");
        run(&path, seed(), |app| {
            press(app, "jash");
            assert_eq!(titles(app), ["dead code"], "pizza left Home at once");
            press(app, "z");
        });
        assert_eq!(fs::read_to_string(&path).unwrap(), HIDDEN);

        let app = run(&path, seed(), |app| {
            let pizza = find(app, "pizza");
            assert_eq!(
                (pizza.score, pizza.vote, pizza.saved, pizza.hidden),
                (71, Vote::Up, true, true)
            );
            assert!(!find(app, "dead code").hidden);
            assert!(!find(app, "biscuit").hidden);
            assert_eq!(titles(app), ["dead code"], "Home's first build");
            press(app, "]]]");
            assert_eq!(app.feed, Feed::Saved);
            assert!(app.view.is_empty(), "the saved post is hidden");
            press(app, "]");
            assert_eq!(app.feed, Feed::Hidden);
            assert_eq!(titles(app), ["pizza"]);
        });
        assert_eq!(encode(&app, &seed_joined(&seed())), HIDDEN);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            HIDDEN,
            "written back the same"
        );
    }

    #[test]
    fn unhiding_everything_goes_back_to_version_1() {
        let dir = TempDir::new("unhide");
        let path = dir.0.join("state");
        fs::write(&path, HIDDEN).unwrap();
        run(&path, seed(), |app| {
            press(app, "]]]]");
            assert_eq!((app.feed, titles(app)), (Feed::Hidden, vec!["pizza"]));
            press(app, "h");
            assert!(app.view.is_empty(), "pizza left Hidden at once");
        });
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "workit-state 1\n\
             vote\tdown\tEngineering\tdana\tdead code\n\
             saved\tFood\tchef\tpizza\n\
             vote\tup\tFood\tchef\tpizza\n"
        );
        let app = run(&path, seed(), |app| {
            assert!(!find(app, "pizza").hidden);
            assert_eq!(titles(app), ["dead code", "pizza"], "back on Home");
        });
        assert!(!encode(&app, &seed_joined(&seed())).contains("hidden"));
    }

    #[test]
    fn a_hidden_record_in_a_version_1_file_is_malformed() {
        let records = "vote\tup\tEngineering\tdana\tdead code\n\
            hidden\tFood\tchef\tpizza\n\
            saved\tFood\tchef\tpizza\n";
        let mut app = seed();
        let v1 = format!("workit-state 1\n{records}");
        assert_eq!(apply(&mut app, v1.as_bytes()), Ok(1), "skipped and counted");
        let pizza = find(&app, "pizza");
        assert_eq!((pizza.saved, pizza.hidden), (true, false));
        assert_eq!(find(&app, "dead code").score, 101, "the rest loads");

        let mut app = seed();
        let v2 = format!("workit-state 2\n{records}");
        assert_eq!(apply(&mut app, v2.as_bytes()), Ok(0));
        assert!(find(&app, "pizza").hidden);

        // In a version 2 file, a `hidden` record is still checked like the rest.
        let mut app = seed();
        let bad = "workit-state 2\n\
            hidden\tFood\tchef\n\
            hidden\tFood\tchef\tpizza\textra\n\
            hidden\tyes\tFood\tchef\tpizza\n\
            hidden\tFood\tchef\tpasta\n";
        assert_eq!(
            apply(&mut app, bad.as_bytes()),
            Ok(3),
            "pasta matches nothing"
        );
        assert!(app.posts.iter().all(|p| !p.hidden));

        let dir = TempDir::new("hidden-v1");
        let path = dir.0.join("state");
        fs::write(&path, &v1).unwrap();
        let mut app = seed();
        let (store, message) = Store::load(path.clone(), &mut app);
        assert!(message.unwrap().starts_with("ignored 1 malformed line(s)"));
        store.save(&app).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "workit-state 1\n\
             vote\tup\tEngineering\tdana\tdead code\n\
             saved\tFood\tchef\tpizza\n",
            "the next save drops it"
        );
    }

    #[test]
    fn a_version_1_file_is_written_back_byte_for_byte() {
        let dir = TempDir::new("v1-unchanged");
        let path = dir.0.join("state");
        fs::write(&path, CHANGED).unwrap();
        let app = run(&path, seed(), |_| {});
        assert_eq!(fs::read_to_string(&path).unwrap(), CHANGED);
        assert_eq!(encode(&app, &seed_joined(&seed())), CHANGED);
    }

    #[test]
    fn a_future_version_with_hidden_records_is_left_alone() {
        let dir = TempDir::new("version-4");
        let path = dir.0.join("state");
        let contents = "workit-state 4\n\
            hidden\tFood\tchef\tpizza\n\
            read\tFood\tchef\tpizza\n\
            saved\tFood\tchef\tpizza\n";
        fs::write(&path, contents).unwrap();
        let mut app = seed();
        let (store, message) = Store::load(path.clone(), &mut app);
        let message = message.unwrap();
        assert!(message.contains("left unchanged"), "{message}");
        assert!(message.contains("\"workit-state 1\", \"workit-state 2\" or \"workit-state 3\""));
        let pizza = find(&app, "pizza");
        assert_eq!(
            (pizza.saved, pizza.hidden, pizza.read),
            (false, false, false)
        );
        press(&mut app, "j\nqh");
        store.save(&app).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), contents);
        assert_eq!(dir.entries(), ["state"]);
    }

    fn esc(app: &mut App) {
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    }

    /// The same votes on "dead code" with and without your comments: a
    /// downvote on lin (depth 2) and an upvote on sam. With them, a comment
    /// of yours goes first (pushing marco and sam down one), downvoted; a
    /// reply to dana goes before lin; and you reply to your own reply.
    fn vote_with_comments(app: &mut App) {
        press(app, "\nrfirst!\nz");
        assert_eq!(app.posts[0].comments[0].author, "you");
        press(app, "nnrme too\nrand me\n");
        assert_eq!(app.posts[0].comments[1].replies[0].replies[0].author, "you");
        press(app, "nzna");
        let dead = &app.posts[0];
        let lin = &dead.comments[1].replies[0].replies[1];
        assert_eq!(
            (lin.author, lin.vote),
            ("lin", Vote::Down),
            "pushed down one"
        );
        assert_eq!(
            (dead.comments[2].author, dead.comments[2].vote),
            ("sam", Vote::Up)
        );
        assert_eq!(dead.comment_count, 3);
    }

    fn vote_without_comments(app: &mut App) {
        press(app, "\nnnnznaq");
    }

    /// Both runs open "dead code", which marks it read.
    const VOTED: &str = "workit-state 3\n\
        read\tEngineering\tdana\tdead code\n\
        vote\tdown\tEngineering\tdana\tdead code\tmarco\tcongrats\tdana\tthanks\tlin\tdeep\n\
        vote\tup\tEngineering\tdana\tdead code\tsam\tbroke?\n";

    #[test]
    fn your_comments_leave_the_saved_file_byte_for_byte_the_same() {
        let dir = TempDir::new("your-comments");
        let with = dir.0.join("with");
        let without = dir.0.join("without");
        run(&with, seed(), vote_with_comments);
        run(&without, seed(), vote_without_comments);
        let saved = fs::read(&with).unwrap();
        assert_eq!(saved, fs::read(&without).unwrap());
        assert_eq!(String::from_utf8(saved).unwrap(), VOTED);

        // Loading restores the seeded votes onto the right comments, and none
        // of yours: every run starts with the seed tree.
        let app = run(&with, seed(), |_| {});
        let dead = find(&app, "dead code");
        assert_eq!(dead.comments.len(), 2);
        assert_eq!(dead.comment_count, seed().posts[0].comment_count);
        let marco = &dead.comments[0];
        assert_eq!(marco.replies[0].replies.len(), 1);
        let lin = &marco.replies[0].replies[0];
        assert_eq!((lin.author, lin.score, lin.vote), ("lin", 29, Vote::Down));
        let sam = &dead.comments[1];
        assert_eq!((sam.author, sam.score, sam.vote), ("sam", 41, Vote::Up));
        assert_eq!((marco.score, marco.vote), (10, Vote::None));
        assert!(items(&app).iter().all(|(key, _)| !key.contains(&"you")));
        assert_eq!(
            fs::read_to_string(&with).unwrap(),
            VOTED,
            "written back the same"
        );
    }

    #[test]
    fn writing_comments_alone_saves_nothing() {
        let dir = TempDir::new("only-comments");
        let path = dir.0.join("state");
        run(&path, seed(), |app| {
            press(app, "\nrhello\nrback at you\n");
            esc(app);
            // Opening the post marked it read; `m` undoes that, so the
            // comments are all that is left.
            press(app, "rtop again\nmq");
        });
        assert_eq!(dir.entries(), Vec::<String>::new(), "no file created");
        assert_eq!(encode(&seed(), &seed_joined(&seed())), "workit-state 1\n");
    }

    #[test]
    fn loading_never_touches_your_comments() {
        let mut app = seed();
        vote_with_comments(&mut app);
        let mine = |app: &App| {
            let dead = &app.posts[0];
            let reply = &dead.comments[1].replies[0].replies[0];
            (
                dead.comments[0].clone(),
                reply.clone(),
                dead.comments.len(),
                dead.comment_count,
            )
        };
        let before = format!("{:?}", mine(&app));
        // Records naming your comments' keys match nothing; the seeded ones
        // land on the seeded comments where they are now.
        let file = "workit-state 1\n\
            vote\tup\tEngineering\tdana\tdead code\tyou\tfirst!\n\
            vote\tup\tEngineering\tdana\tdead code\tmarco\tcongrats\tdana\tthanks\tyou\tme too\n\
            vote\tup\tEngineering\tdana\tdead code\tmarco\tcongrats\n";
        assert_eq!(apply(&mut app, file.as_bytes()), Ok(0));
        assert_eq!(format!("{:?}", mine(&app)), before);
        let marco = &app.posts[0].comments[1];
        assert_eq!(
            (marco.author, marco.score, marco.vote),
            ("marco", 11, Vote::Up)
        );
    }

    #[test]
    fn your_comment_never_takes_a_seeded_siblings_key() {
        // A seeded comment by someone whose name is `you`, so that one of
        // yours with the same text has exactly its key.
        let same = || {
            let groups = vec![group("Engineering", true)];
            let posts = vec![post(
                &groups,
                "Engineering",
                "dana",
                "dead code",
                100,
                1,
                vec![c("you", 5, 1, "same")],
            )];
            App::with_data(groups, posts)
        };
        let mut app = same();
        // Yours goes first and stays upvoted; the seeded one is downvoted.
        press(&mut app, "\nrsame\nnz");
        let comments = &app.posts[0].comments;
        assert!(comments[0].mine && !comments[1].mine);
        assert_eq!(
            (comments[0].body.as_ref(), comments[0].vote),
            ("same", Vote::Up)
        );
        assert_eq!(
            (comments[1].body.as_ref(), comments[1].vote),
            ("same", Vote::Down)
        );
        let text = encode(&app, &seed_joined(&same()));
        assert_eq!(
            text,
            "workit-state 3\n\
             read\tEngineering\tdana\tdead code\n\
             vote\tdown\tEngineering\tdana\tdead code\tyou\tsame\n",
            "the post opened, and only the seeded comment's vote, under its key"
        );

        let mut fresh = same();
        assert_eq!(apply(&mut fresh, text.as_bytes()), Ok(0));
        let seeded = &fresh.posts[0].comments[0];
        assert_eq!((seeded.score, seeded.vote), (4, Vote::Down));

        // Loaded into a run with your comment first, it still lands on the
        // seeded one.
        let mut again = same();
        press(&mut again, "\nrsame\n");
        assert_eq!(apply(&mut again, text.as_bytes()), Ok(0));
        let comments = &again.posts[0].comments;
        assert_eq!((comments[0].score, comments[0].vote), (1, Vote::Up));
        assert_eq!((comments[1].score, comments[1].vote), (4, Vote::Down));
    }

    // ---- Read posts ----

    #[test]
    fn a_read_post_round_trips_with_version_3() {
        let dir = TempDir::new("read");
        let path = dir.0.join("state");
        run(&path, seed(), |app| press(app, "j\nq"));
        let read = "workit-state 3\nread\tFood\tchef\tpizza\n";
        assert_eq!(fs::read_to_string(&path).unwrap(), read);

        let app = run(&path, seed(), |app| {
            assert!(find(app, "pizza").read);
            assert!(!find(app, "dead code").read && !find(app, "biscuit").read);
            assert_eq!(titles(app), ["dead code", "pizza"], "reading hides nothing");
            press(app, "/is:read\n");
            assert_eq!(titles(app), ["pizza"]);
        });
        assert_eq!(encode(&app, &seed_joined(&seed())), read);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            read,
            "written back the same"
        );
    }

    /// "dead code" downvoted, opened and sam's comment on it upvoted;
    /// "pizza" upvoted, saved, opened and hidden. By key presses on Home.
    const READ_ALL: &str = "workit-state 3\n\
        read\tEngineering\tdana\tdead code\n\
        vote\tdown\tEngineering\tdana\tdead code\n\
        vote\tup\tEngineering\tdana\tdead code\tsam\tbroke?\n\
        saved\tFood\tchef\tpizza\n\
        hidden\tFood\tchef\tpizza\n\
        read\tFood\tchef\tpizza\n\
        vote\tup\tFood\tchef\tpizza\n";

    #[test]
    fn a_read_hidden_saved_and_voted_post_round_trips_with_all_four() {
        let dir = TempDir::new("read-all");
        let path = dir.0.join("state");
        run(&path, seed(), |app| {
            press(app, "z\nnnnnaq");
            press(app, "jas\nqh");
            assert_eq!(titles(app), ["dead code"]);
        });
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            READ_ALL,
            "saved, hidden, read, the post's vote, then its comments' votes"
        );

        let app = run(&path, seed(), |app| {
            let pizza = find(app, "pizza");
            assert_eq!(
                (
                    pizza.score,
                    pizza.vote,
                    pizza.saved,
                    pizza.hidden,
                    pizza.read
                ),
                (71, Vote::Up, true, true, true)
            );
            let dead = find(app, "dead code");
            assert_eq!((dead.score, dead.vote, dead.read), (99, Vote::Down, true));
            assert_eq!(dead.comments[1].vote, Vote::Up);
            assert!(!find(app, "biscuit").read);
            assert_eq!(titles(app), ["dead code"], "pizza is hidden");
            press(app, "]]]]");
            assert_eq!((app.feed, titles(app)), (Feed::Hidden, vec!["pizza"]));
        });
        assert_eq!(encode(&app, &seed_joined(&seed())), READ_ALL);
        assert_eq!(fs::read_to_string(&path).unwrap(), READ_ALL);
    }

    #[test]
    fn marking_everything_unread_goes_back_to_version_2_or_1() {
        let dir = TempDir::new("unread");
        let path = dir.0.join("state");
        fs::write(&path, READ_ALL).unwrap();
        run(&path, seed(), |app| {
            press(app, "m");
            assert_eq!(app.status, Some("Marked unread"));
            press(app, "]]]]m");
            assert_eq!(app.feed, Feed::Hidden);
            assert!(app.posts.iter().all(|p| !p.read));
        });
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "workit-state 2\n\
             vote\tdown\tEngineering\tdana\tdead code\n\
             vote\tup\tEngineering\tdana\tdead code\tsam\tbroke?\n\
             saved\tFood\tchef\tpizza\n\
             hidden\tFood\tchef\tpizza\n\
             vote\tup\tFood\tchef\tpizza\n",
            "a hidden post is left, so version 2"
        );

        // With nothing hidden, back to version 1.
        fs::write(
            &path,
            "workit-state 3\n\
             read\tEngineering\tdana\tdead code\n\
             saved\tFood\tchef\tpizza\n\
             read\tFood\tchef\tpizza\n",
        )
        .unwrap();
        run(&path, seed(), |app| {
            assert!(find(app, "dead code").read && find(app, "pizza").read);
            // Opened again and marked unread in the post view, and `m` in
            // the feed.
            press(app, "\nmqjm");
        });
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "workit-state 1\nsaved\tFood\tchef\tpizza\n"
        );
        let app = run(&path, seed(), |_| {});
        assert!(app.posts.iter().all(|p| !p.read));
    }

    #[test]
    fn a_read_record_in_a_version_1_or_2_file_is_malformed() {
        let records = "vote\tup\tEngineering\tdana\tdead code\n\
            read\tFood\tchef\tpizza\n\
            saved\tFood\tchef\tpizza\n\
            hidden\tDogs\tkim\tbiscuit\n";
        // Version 1: `read` and `hidden` are both malformed.
        let mut app = seed();
        let v1 = format!("workit-state 1\n{records}");
        assert_eq!(apply(&mut app, v1.as_bytes()), Ok(2), "skipped and counted");
        let pizza = find(&app, "pizza");
        assert_eq!((pizza.saved, pizza.read), (true, false));
        assert!(!find(&app, "biscuit").hidden);
        assert_eq!(find(&app, "dead code").score, 101, "the rest loads");

        // Version 2: only `read` is.
        let mut app = seed();
        let v2 = format!("workit-state 2\n{records}");
        assert_eq!(apply(&mut app, v2.as_bytes()), Ok(1));
        assert!(!find(&app, "pizza").read && find(&app, "biscuit").hidden);

        // Version 3: both load.
        let mut app = seed();
        let v3 = format!("workit-state 3\n{records}");
        assert_eq!(apply(&mut app, v3.as_bytes()), Ok(0));
        assert!(find(&app, "pizza").read && find(&app, "biscuit").hidden);

        // In a version 3 file a `read` record is still checked like the rest,
        // and one that matches nothing is dropped.
        let mut app = seed();
        let bad = "workit-state 3\n\
            read\tFood\tchef\n\
            read\tFood\tchef\tpizza\textra\n\
            read\tyes\tFood\tchef\tpizza\n\
            read\tFood\tchef\tpasta\n\
            read\tEngineering\tdana\tdead code\tsam\tbroke?\n";
        assert_eq!(
            apply(&mut app, bad.as_bytes()),
            Ok(4),
            "pasta matches nothing"
        );
        assert!(app.posts.iter().all(|p| !p.read));
        assert_eq!(encode(&app, &seed_joined(&seed())), "workit-state 1\n");

        // The next save drops a malformed `read` line.
        for (file, saved) in [
            (
                &v1,
                "workit-state 1\n\
                 vote\tup\tEngineering\tdana\tdead code\n\
                 saved\tFood\tchef\tpizza\n",
            ),
            (
                &v2,
                "workit-state 2\n\
                 vote\tup\tEngineering\tdana\tdead code\n\
                 hidden\tDogs\tkim\tbiscuit\n\
                 saved\tFood\tchef\tpizza\n",
            ),
        ] {
            let dir = TempDir::new("read-v1-v2");
            let path = dir.0.join("state");
            fs::write(&path, file).unwrap();
            let mut app = seed();
            let (store, message) = Store::load(path.clone(), &mut app);
            assert!(message.unwrap().contains("malformed line(s)"));
            store.save(&app).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), saved);
        }
    }

    #[test]
    fn version_2_and_3_files_are_written_back_byte_for_byte() {
        for (name, contents) in [("v2", HIDDEN), ("v3", READ_ALL)] {
            let dir = TempDir::new(&format!("{name}-unchanged"));
            let path = dir.0.join("state");
            fs::write(&path, contents).unwrap();
            // Moving around, searching and switching views reads no post.
            let app = run(&path, seed(), |app| press(app, "jkv]]/x\x1b"));
            assert_eq!(fs::read_to_string(&path).unwrap(), contents, "{name}");
            assert_eq!(encode(&app, &seed_joined(&seed())), contents, "{name}");
        }
    }

    #[test]
    fn read_state_follows_the_post_key_through_a_reordered_seed() {
        // "pizza" and "biscuit" read, plus a `read` record for a post that
        // isn't in the new seed.
        let mut app = seed();
        app.posts[1].read = true;
        app.posts[2].read = true;
        let text = format!(
            "{}read\tFood\tchef\tpasta\n",
            encode(&app, &seed_joined(&seed()))
        );
        assert_eq!(
            text,
            "workit-state 3\n\
             read\tDogs\tkim\tbiscuit\n\
             read\tFood\tchef\tpizza\n\
             read\tFood\tchef\tpasta\n"
        );

        let mut app = reordered_seed();
        let seed_joined = seed_joined(&app);
        assert_eq!(
            apply(&mut app, text.as_bytes()),
            Ok(0),
            "pasta isn't malformed"
        );
        let read: Vec<&str> = app
            .posts
            .iter()
            .filter(|p| p.read)
            .map(|p| p.title)
            .collect();
        assert_eq!(read, ["pizza", "biscuit"], "in the new seed order");
        assert!(!find(&app, "new post").read && !find(&app, "dead code").read);
        assert_eq!(
            encode(&app, &seed_joined),
            "workit-state 3\n\
             read\tFood\tchef\tpizza\n\
             read\tDogs\tkim\tbiscuit\n",
            "the new seed order, and the record matching nothing dropped"
        );
    }
}
