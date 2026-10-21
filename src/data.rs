//! Seed content. Every group, person and post here is fictional.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Vote {
    Up,
    #[default]
    None,
    Down,
}

#[derive(Debug, Clone)]
pub(crate) struct Group {
    pub name: &'static str,
    pub description: &'static str,
    pub members: u32,
    pub online: u32,
    pub joined: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Comment {
    pub author: &'static str,
    pub body: &'static str,
    pub score: i32,
    pub hours_ago: u32,
    pub replies: Vec<Comment>,
    pub vote: Vote,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Post {
    pub group: usize,
    pub author: &'static str,
    pub title: &'static str,
    pub body: &'static str,
    pub flair: Option<&'static str>,
    pub score: i32,
    pub hours_ago: u32,
    /// The comment tree only holds a handful of seeded comments; this is the
    /// larger count shown on the card, as most threads are collapsed on Reddit.
    pub comment_count: u32,
    pub comments: Vec<Comment>,
    pub vote: Vote,
    pub saved: bool,
}

fn c(author: &'static str, score: i32, hours_ago: u32, body: &'static str) -> Comment {
    Comment {
        author,
        body,
        score,
        hours_ago,
        replies: Vec::new(),
        vote: Vote::None,
    }
}

impl Comment {
    /// This comment plus all of its replies, recursively.
    pub(crate) fn total(&self) -> u32 {
        1 + self.replies.iter().map(Comment::total).sum::<u32>()
    }

    fn with(mut self, replies: Vec<Comment>) -> Self {
        self.replies = replies;
        self
    }
}

const ENG: usize = 0;
const REMOTE: usize = 1;
const OFFICE: usize = 2;
const HACK: usize = 3;
const ANNOUNCE: usize = 4;
const FOOD: usize = 5;
const DOGS: usize = 6;
const ONCALL: usize = 7;

pub(crate) fn groups() -> Vec<Group> {
    let g = |name, description, members, online, joined| Group {
        name,
        description,
        members,
        online,
        joined,
    };
    vec![
        g(
            "Engineering",
            "Code, craft, and the eternal tabs-vs-spaces debate.",
            48_210,
            1_204,
            true,
        ),
        g(
            "RemoteWork",
            "For everyone whose commute is from the bed to the desk.",
            21_877,
            612,
            true,
        ),
        g(
            "OfficeLife",
            "Microkitchens, meeting rooms, and the mystery of floor 3's thermostat.",
            30_455,
            820,
            true,
        ),
        g(
            "Hackathon",
            "Ship something weird in 48 hours. Sleep is optional.",
            12_903,
            341,
            true,
        ),
        g(
            "Announcements",
            "Official news. Please read before asking in the comments.",
            96_002,
            2_310,
            false,
        ),
        g(
            "Foodies",
            "Cafe menu reviews and strong opinions about the salad bar.",
            18_340,
            455,
            true,
        ),
        g(
            "DogsOfTheOffice",
            "Good boys and girls with badges. Pics required.",
            25_118,
            903,
            false,
        ),
        g(
            "OncallSurvivors",
            "Support group for those who have been paged at 3am.",
            9_761,
            288,
            true,
        ),
    ]
}

pub(crate) fn posts() -> Vec<Post> {
    vec![
        Post {
            group: ENG,
            author: "dana_k",
            hours_ago: 3,
            score: 1_243,
            flair: Some("Win"),
            title: "We deleted 40k lines of dead code and the build got 30% faster",
            body: "After six months of slowly untangling flags that had been fully rolled out \
                   since 2021, the team finally removed the last of them. Build times dropped \
                   from 11 minutes to under 8. Huge thanks to everyone who reviewed the 212 diffs.",
            comment_count: 194,
            comments: vec![
                c(
                    "marco_p",
                    402,
                    3,
                    "The best code is no code. Congrats team!",
                )
                .with(vec![c(
                    "dana_k",
                    120,
                    2,
                    "Next goal: delete the code that deletes the code.",
                )]),
                c(
                    "lin_tran",
                    256,
                    2,
                    "212 diffs?! Who reviewed all of these, and are they okay?",
                )
                .with(vec![
                    c("dana_k", 98, 2, "They are resting now."),
                    c(
                        "oz_reviewer",
                        87,
                        1,
                        "I have seen things you people wouldn't believe.",
                    ),
                ]),
                c(
                    "sam_builds",
                    61,
                    1,
                    "Did anything break? Asking for a friend who owns a downstream service.",
                ),
            ],
            ..Default::default()
        },
        Post {
            group: OFFICE,
            author: "priya_r",
            hours_ago: 1,
            score: 856,
            flair: Some("PSA"),
            title: "PSA: the espresso machine on floor 3 is FIXED",
            body: "Facilities replaced the grinder this morning. It now makes coffee instead of \
                   angry grinding noises. Please stop putting oat milk in the regular milk pitcher.",
            comment_count: 100,
            comments: vec![
                c(
                    "jules_w",
                    310,
                    1,
                    "This is the most important post of the quarter.",
                ),
                c(
                    "nate_o",
                    144,
                    0,
                    "Productivity on floor 3 expected to rise 400%.",
                )
                .with(vec![c(
                    "priya_r",
                    60,
                    0,
                    "Productivity on floor 2 expected to fall as they all walk up.",
                )]),
            ],
            ..Default::default()
        },
        Post {
            group: ANNOUNCE,
            author: "comms_team",
            hours_ago: 5,
            score: 2_410,
            flair: Some("Official"),
            title: "Hackathon 2026 is happening October 22-24 — registration is open",
            body: "Form a team of up to five people and build anything. Demos will be on Friday \
                   afternoon, and there will be prizes across six categories, including 'Most \
                   Likely To Actually Ship'. Food will be provided all three days.",
            comment_count: 545,
            comments: vec![
                c(
                    "hack_enjoyer",
                    720,
                    5,
                    "Already have 4 ideas and 0 teammates. Who's in?",
                )
                .with(vec![
                    c(
                        "ravi_m",
                        210,
                        4,
                        "I'm in if one of the ideas involves the coffee machine.",
                    ),
                    c("hack_enjoyer", 88, 4, "All four of them do, actually."),
                ]),
                c(
                    "quiet_pm",
                    190,
                    4,
                    "Is 'Most Likely To Actually Ship' a challenge or a threat?",
                ),
            ],
            ..Default::default()
        },
        Post {
            group: REMOTE,
            author: "alex_from_home",
            hours_ago: 7,
            score: 634,
            flair: Some("Discussion"),
            title: "What's your best trick for not going stir-crazy working remote?",
            body: "I've been fully remote for two years. Lately the days blur together. I've \
                   tried standing desks, pomodoro, and talking to my plants. What actually works \
                   for you?",
            comment_count: 215,
            comments: vec![
                c(
                    "mia_s",
                    330,
                    6,
                    "Fake commute. I walk around the block before and after work. Sounds silly, \
                     works great.",
                )
                .with(vec![c(
                    "alex_from_home",
                    75,
                    6,
                    "Trying this tomorrow, thank you.",
                )]),
                c(
                    "ben_t",
                    201,
                    6,
                    "Camera-on coffee chats with random people from other teams. Met half my \
                     friends this way.",
                ),
                c(
                    "plant_parent",
                    96,
                    5,
                    "Talking to your plants is valid and they are listening.",
                ),
            ],
            ..Default::default()
        },
        Post {
            group: DOGS,
            author: "kim_and_biscuit",
            hours_ago: 2,
            score: 3_120,
            flair: Some("Pics"),
            title: "Biscuit got his own visitor badge today",
            body: "Security printed him a badge because he 'kept tailgating'. He is very proud. \
                   Photo is in the comments (he is wearing it upside down).",
            comment_count: 406,
            comments: vec![
                c("doglover99", 980, 2, "BISCUIT. EMPLOYEE OF THE MONTH."),
                c(
                    "security_sam",
                    640,
                    1,
                    "Can confirm. Repeat offender. Very good boy.",
                )
                .with(vec![c(
                    "kim_and_biscuit",
                    300,
                    1,
                    "He says thank you for your service.",
                )]),
            ],
            ..Default::default()
        },
        Post {
            group: ONCALL,
            author: "tired_sre",
            hours_ago: 9,
            score: 1_877,
            flair: Some("Rant"),
            title: "Got paged at 3:12am because a dashboard had a typo in its alert threshold",
            body: "The threshold was 0.5 instead of 50. It has been firing in a channel nobody \
                   reads for eight months. Last night someone 'fixed' the routing. I am typing \
                   this from the floor.",
            comment_count: 305,
            comments: vec![
                c(
                    "oncall_vet",
                    700,
                    8,
                    "Classic. The alert was always right, we just didn't listen.",
                ),
                c(
                    "lin_tran",
                    410,
                    8,
                    "Delete the alert. Write a postmortem. Delete the postmortem. Sleep.",
                )
                .with(vec![c("tired_sre", 190, 7, "This is the way.")]),
            ],
            ..Default::default()
        },
        Post {
            group: FOOD,
            author: "chef_critic",
            hours_ago: 4,
            score: 512,
            flair: Some("Review"),
            title: "Ranking every cafe pizza from this week (it's not close)",
            body: "1. Tuesday's mushroom truffle. Transcendent.\n2. Monday's margherita. \
                   Classic, reliable.\n3. Thursday's pineapple jalapeño. Divisive but brave.\n4. \
                   Wednesday's 'breakfast pizza'. We do not speak of Wednesday.",
            comment_count: 154,
            comments: vec![
                c(
                    "pizza_purist",
                    220,
                    4,
                    "Pineapple jalapeño at #3 is a crime.",
                ),
                c(
                    "night_shift",
                    130,
                    3,
                    "Wednesday's had an egg on it. AN EGG.",
                )
                .with(vec![c(
                    "chef_critic",
                    70,
                    3,
                    "We said we would not speak of it.",
                )]),
            ],
            ..Default::default()
        },
        Post {
            group: HACK,
            author: "ravi_m",
            hours_ago: 11,
            score: 298,
            flair: Some("Looking for team"),
            title: "LF 2 people for a hackathon project: a TUI that makes Workplace look like Reddit",
            body: "Hear me out. Vote arrows. Sort by Hot. Threaded comments. All in your \
                   terminal. Need someone who knows ratatui and someone who can write fake posts.",
            comment_count: 42,
            comments: vec![
                c(
                    "terminal_tess",
                    140,
                    10,
                    "This is the most cursed thing I've ever wanted. DM'd.",
                ),
                c("meta_irony", 99, 9, "Is this post... inside the app?"),
            ],
            ..Default::default()
        },
        Post {
            group: ENG,
            author: "grumpy_staff_eng",
            hours_ago: 14,
            score: 921,
            flair: Some("Discussion"),
            title: "Unpopular opinion: most of our microservices should be one service",
            body: "We have 14 services that all call each other synchronously, share a database, \
                   and deploy together. That's a monolith with extra network hops. Change my mind.",
            comment_count: 393,
            comments: vec![
                c(
                    "arch_astronaut",
                    300,
                    13,
                    "Counterpoint: if they were one service, who would we blame in postmortems?",
                ),
                c(
                    "dana_k",
                    260,
                    12,
                    "A distributed monolith is the worst of both worlds and I've lived in three.",
                )
                .with(vec![c(
                    "grumpy_staff_eng",
                    120,
                    12,
                    "Rent is high in those neighborhoods.",
                )]),
            ],
            ..Default::default()
        },
        Post {
            group: OFFICE,
            author: "thermo_warrior",
            hours_ago: 20,
            score: 433,
            flair: Some("Rant"),
            title: "Who keeps setting the 4th floor thermostat to 64°F",
            body: "I've started bringing a blanket. Then a second blanket. I'm now considering a \
                   small personal sauna. Please, whoever you are, we can talk about this.",
            comment_count: 122,
            comments: vec![
                c(
                    "arctic_fox",
                    190,
                    19,
                    "It's me. I run hot. I'm sorry. (I'm not sorry.)",
                ),
                c("thermo_warrior", 150, 19, "I knew it."),
            ],
            ..Default::default()
        },
        Post {
            group: REMOTE,
            author: "nomad_nina",
            hours_ago: 30,
            score: 1_402,
            flair: Some("Story"),
            title: "My cat joined a 200-person all-hands by walking across my keyboard and unmuting me",
            body: "She then sat on the keyboard, which apparently triggered a hand raise. The \
                   host called on me. I had to explain. The cat has now been asked back for next \
                   month.",
            comment_count: 262,
            comments: vec![
                c(
                    "hr_holly",
                    600,
                    29,
                    "Speaking as the host: she was the highlight.",
                ),
                c("ben_t", 220, 28, "We need a w/CatsOfTheOffice now."),
            ],
            ..Default::default()
        },
        Post {
            group: ANNOUNCE,
            author: "comms_team",
            hours_ago: 26,
            score: 1_105,
            flair: Some("Official"),
            title: "New building opens Monday: here's everything you need to know",
            body: "Badge access goes live at 7am. The cafe on level 2 opens at 8. Parking is in \
                   garage C. There is a slide between floors 3 and 2. Yes, really. No, you may not \
                   bring a skateboard.",
            comment_count: 332,
            comments: vec![
                c(
                    "skate_sk8",
                    400,
                    25,
                    "What about a scooter? Asking for legal reasons.",
                )
                .with(vec![c("comms_team", 300, 24, "No.")]),
            ],
            ..Default::default()
        },
        Post {
            group: ONCALL,
            author: "first_time_oncall",
            hours_ago: 6,
            score: 388,
            flair: Some("Help"),
            title: "First oncall shift starts tomorrow. Any advice?",
            body: "I've read the runbook three times. I've set up alerts on my phone. I've \
                   stocked up on snacks. What else should I know?",
            comment_count: 113,
            comments: vec![
                c(
                    "oncall_vet",
                    210,
                    5,
                    "Write down everything you do while it's happening. Future you will thank you.",
                ),
                c(
                    "tired_sre",
                    180,
                    5,
                    "Escalating is not failing. Escalate early, escalate often.",
                )
                .with(vec![c(
                    "first_time_oncall",
                    60,
                    4,
                    "Writing this on a sticky note.",
                )]),
            ],
            ..Default::default()
        },
        Post {
            group: HACK,
            author: "terminal_tess",
            hours_ago: 40,
            score: 744,
            flair: Some("Demo"),
            title: "Our team built a bot that writes your status update from your calendar",
            body: "It looks at your meetings for the week and writes 'drove alignment across \
                   stakeholders' for each one. Accuracy: concerning. Judges' reaction: nervous \
                   laughter.",
            comment_count: 151,
            comments: vec![c(
                "quiet_pm",
                330,
                38,
                "Please don't ship this, I'll be out of a job.",
            )],
            ..Default::default()
        },
        Post {
            group: FOOD,
            author: "salad_stan",
            hours_ago: 16,
            score: 267,
            title: "The salad bar added a 'build your own grain bowl' station and I've lost all \
                    self-control",
            body: "Day 4 of eating a bowl that is 70% toppings. Send help (and more feta).",
            comment_count: 61,
            comments: vec![c(
                "chef_critic",
                80,
                15,
                "The feta ratio is a personal choice.",
            )],
            ..Default::default()
        },
        Post {
            group: DOGS,
            author: "corgi_dad",
            hours_ago: 18,
            score: 1_990,
            flair: Some("Pics"),
            title: "Waffles attended his first sprint planning and estimated everything as 'treat'",
            body: "Honestly more accurate than our usual estimates.",
            comment_count: 211,
            comments: vec![c(
                "scrum_master_jo",
                700,
                17,
                "Story points are now measured in treats. Effective immediately.",
            )],
            ..Default::default()
        },
        Post {
            group: ENG,
            author: "new_grad_noah",
            hours_ago: 2,
            score: 156,
            flair: Some("Question"),
            title: "Is it normal to feel lost during the first month?",
            body: "Everyone uses acronyms I don't know and the codebase is enormous. My mentor is \
                   great but I don't want to bother them with every little question.",
            comment_count: 73,
            comments: vec![
                c(
                    "grumpy_staff_eng",
                    300,
                    2,
                    "Ten years in and I still feel lost sometimes. Ask the questions. Bother your \
                     mentor. That's what they're for.",
                )
                .with(vec![c(
                    "new_grad_noah",
                    90,
                    1,
                    "This genuinely helps. Thank you.",
                )]),
                c(
                    "lin_tran",
                    110,
                    1,
                    "Keep a doc of every acronym you learn. Share it. You'll be a hero.",
                ),
            ],
            ..Default::default()
        },
        Post {
            group: OFFICE,
            author: "booked_solid",
            hours_ago: 50,
            score: 678,
            flair: Some("Meme"),
            title: "Every meeting room booked from 9 to 5 and every one of them is empty",
            body: "I've walked the whole floor. Room 'Pluto' has had a recurring reservation \
                   since 2019 for a meeting called 'sync'. Nobody has ever been seen inside.",
            comment_count: 181,
            comments: vec![c(
                "ghost_meeting",
                410,
                48,
                "I am the organizer. I cannot cancel it. I have tried.",
            )],
            ..Default::default()
        },
        Post {
            group: REMOTE,
            author: "time_zone_tom",
            hours_ago: 1,
            score: 92,
            flair: Some("Discussion"),
            title: "To everyone scheduling 8am Pacific meetings: some of us are in Sydney",
            body: "That's 2am here. I love you all, but please check the time zone helper.",
            comment_count: 19,
            comments: vec![c(
                "mia_s",
                40,
                0,
                "Recurring meetings should rotate time zones. Fight me.",
            )],
            ..Default::default()
        },
        Post {
            group: ANNOUNCE,
            author: "benefits_team",
            hours_ago: 60,
            score: 845,
            flair: Some("Official"),
            title: "Open enrollment starts next week — here's what's new",
            body: "Highlights: expanded mental health coverage, a new fertility benefit, and an \
                   increased wellness stipend. Info sessions on Tuesday and Thursday.",
            comment_count: 91,
            comments: vec![c(
                "ben_t",
                120,
                58,
                "The wellness stipend increase is very welcome.",
            )],
            ..Default::default()
        },
        Post {
            group: HACK,
            author: "sleepless_sid",
            hours_ago: 3,
            score: 201,
            title: "It is 4am. Our demo is in 6 hours. The code works but only if you don't look \
                    at it",
            body: "If anyone on floor 5 has a spare phone charger and some dignity, please come by.",
            comment_count: 31,
            comments: vec![c(
                "terminal_tess",
                70,
                2,
                "Bringing the charger. Can't help with the dignity.",
            )],
            ..Default::default()
        },
        Post {
            group: ONCALL,
            author: "postmortem_pat",
            hours_ago: 70,
            score: 1_210,
            flair: Some("Story"),
            title: "Postmortem of the week: the outage was caused by a leap second in a test fixture",
            body: "The fixture had a hard-coded date in 2016. It only failed on a specific day of \
                   the year in a specific time zone. Root cause: time is a social construct.",
            comment_count: 201,
            comments: vec![c(
                "oncall_vet",
                300,
                68,
                "Every time bug is eventually a philosophy degree.",
            )],
            ..Default::default()
        },
        Post {
            group: FOOD,
            author: "boba_believer",
            hours_ago: 8,
            score: 1_034,
            flair: Some("PSA"),
            title: "The boba cart is back on Thursdays",
            body: "They've added brown sugar oat milk. Line forms at 2:00pm. Arrive early.",
            comment_count: 91,
            comments: vec![c(
                "night_shift",
                200,
                7,
                "Setting a recurring calendar block. Title: 'sync'.",
            )],
            ..Default::default()
        },
        Post {
            group: ENG,
            author: "flaky_test_fiona",
            hours_ago: 22,
            score: 512,
            flair: Some("Rant"),
            title: "This test has failed 1 in 300 runs for two years and today I found out why",
            body: "It sorted a HashMap's keys by insertion order. It passed whenever the hash \
                   seed was in our favor. Two years. Two. Years.",
            comment_count: 101,
            comments: vec![c(
                "dana_k",
                230,
                21,
                "Nondeterminism: the gift that keeps on giving.",
            )],
            ..Default::default()
        },
    ]
}
