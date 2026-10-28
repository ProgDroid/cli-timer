# Red team: hardening batch design (2026-10-28)

Reviewed: `docs/superpowers/specs/2026-10-28-hardening-batch-design.md`.
Read against: `src/sound.rs` (`FillChannels`, `bound`, `play`), `src/app.rs` (`start_sound`),
`src/cli.rs` (`TuiArgs`), `.github/workflows/release.yml`, `README.md`, `plugin/skills/timer/SKILL.md`,
`docs/parking-lot.md`, `NEXT.md`, and rodio 0.23.0's source in the cargo registry (`source/buffered.rs`,
`source/uniform.rs`, `source/repeat.rs`, `source/take.rs`, `queue.rs`, `player.rs`, `mixer.rs`,
`stream.rs`, `decoder/symphonia.rs`, `conversions/*.rs`, `source/mod.rs`).

Stance: hostile. The question is not "is each item correct" but "is this the right thing to build".

## Evidence: a probe of the real rodio path

The spec's proof for item 1 is a unit test of the number `current_span_len` returns. That number
is consumed by rodio, so I measured what rodio does with it. A scratch crate (outside the repo,
rodio `=0.23.0` with default features off, clap `=4.6.7`) copies `FillChannels` verbatim from
`src/sound.rs`, adds a switch for the proposed formula, and plays it the way cli-timer does:
`Player::connect_new(&mixer)` then `player.append(fill(...))`, where `mixer` is
`rodio::mixer::mixer(channels, rate)`, the same constructor `stream.rs:494` uses for the device
sink. It pulls 2 s from the `MixerSource` (the samples the device would get) and counts output
frames whose channel 0 is wrong. The source is stereo with L = 0.1 and R = 0.9 everywhere, so a
correctly widened 6-channel frame is `[0.1, 0.9, 0.9, 0.9, 0.9, 0.9]`, and any rotation shows up in
channel 0.

Controls, both run first, so the metric is not trusted blind:
- Stereo source to a 2-channel output: 256 bad frames at the start, 0 of 32 256 in the second
  second. The 256 decompose exactly: rodio's first 512 queue samples are converted as mono (see
  below), giving 512 frames `[s, s]` where `s` alternates 0.1/0.9, so 256 have channel 0 = 0.9.
  Non-silent total 80 256 = 512 + (160 000 - 512) / 2. Sums.
- A **native 6-channel** source to a 6-channel output, with no widening (FillChannels passes it
  through): 80 341 of 80 427 frames wrong, and 32 427 of 32 427 in the second second. Decomposed:
  80 427 = 512 head frames + 79 915 frames of the remaining 479 488 samples; the 86 "right" ones are
  the head frames where the mono-misread sample happened to be 0.1 (512 / 6 = 85.3). Sums.

Results (6-channel output at 48 kHz, stereo source widened by `FillChannels`):

| Wiring | Source rate | Today's formula: wrong frames, 2nd second | Proposed formula: wrong frames, 2nd second |
|---|---|---|---|
| `connect_new` then `append` (what cli-timer does) | 48 kHz (no resampling) | 32 427 / 32 427 | 32 427 / 32 427 |
| same | 44.1 kHz (resampling) | 27 539 / 39 486 (70 %) | 39 496 / 39 496 (100 %) |
| same, device pulls one 10 ms period before `append` | 48 kHz | 32 459 / 32 459 | 32 459 / 32 459 |
| same, pre-pull | 44.1 kHz | 27 539 / 39 518 | 39 528 / 39 528 |
| same, stereo source of 4 000 frames (under the 5 461 threshold) | 44.1 kHz | 4 688 / 4 774 (whole sound) | 4 688 / 4 774 |
| `repeat_infinite` (Buffered upstream), as `remind` / the TUI alarm | 44.1 kHz | 49 815 / 72 423 (whole sound) | 72 342 / 72 428 |
| 8-channel (7.1) output | 44.1 kHz | 449 / 22 215 (head only) | 449 / 22 215 |
| `Player::new`, `append`, then `mixer.add(queue)` | 48 kHz | 0 / 32 000 | 0 / 32 000 |
| same | 44.1 kHz | 23 775 / 39 066 | 39 077 / 39 077 |
| `mixer.add(fit_channels(..))` with no `Player` | 48 kHz | 0 / 32 000 | 0 / 32 000 |
| same | 44.1 kHz | 27 178 / 39 066 (first bad frame 5 944 = first 32 768-sample cut) | **0 / 39 076** |

Why (read in the rodio source, then confirmed by the table): `Player::connect_new` calls
`mixer.add(queue_output)` (`player.rs:73-76`) **before** anything is appended.
`UniformSourceIterator::new` bootstraps at once (`uniform.rs:49-66`) from the queue's current
source, `Empty`, which reports 1 channel (`source/empty.rs:44-46`), and from the keep-alive span
`threshold(1) = 512` (`queue.rs:126-131`, `:147-152`). The first 512 samples of the appended sound
are therefore converted as mono to 6 channels, and since 512 = 85 x 6 + 2, every later frame is
rotated by two channels. At equal rates the later chunks are 6-to-6 identity conversions
(`conversions/sample_rate.rs` passes through when `from == to`; `ChannelCountConverter` with
`from == to` just forwards), so nothing ever re-aligns it. With the pre-pull the keep-alive `Zero`
source reports its own 1 channel and 512 again, so the race does not save it either.

What that means for a person on 5.1 (WAVEFORMATEXTENSIBLE order FL FR FC LFE BL BR): a stereo `-s`
file today plays `[R, R, R, R, L, R]`: the left channel comes only out of back-left. The mono
built-ins are unaffected in practice (all channels carry the same sample, so a rotation is
invisible), which is why the NEXT.md check "the alarm plays from every speaker" would pass.

The probe crate lived in the session scratchpad and is not kept; the table above and the wiring
described are enough to rebuild it (about 200 lines).

## Objection 1 (b, assumed rather than established): item 1 fixes a cut that does not decide the output

The spec says rodio's `Buffered` (`source/buffered.rs:112`) caps spans at 32 768 and that this cuts
a widened span mid-frame. Checked:

- `Buffered` appears on cli-timer's path only through `repeat_infinite()` (`repeat.rs:14`), called
  in `bound()` (`src/sound.rs:184`) and `start_sound` (`src/app.rs:208-209`). In both it wraps the
  **inner** decoded source, upstream of `FillChannels`. It caps a 1- or 2-channel stream, where
  32 768 is a whole number of frames. It never sees `FillChannels::current_span_len`.
- The cap that does read `FillChannels`' reported length is `UniformSourceIterator::bootstrap`
  (`uniform.rs:55`, `.min(32768)`), created by `Mixer::add` (`mixer.rs:61-62`). The number
  coincides; the mechanism the spec names does not.
- "32 768 is not a whole number of 6- or 8-channel frames": false for 8. 32 768 = 4 096 x 8. The
  probe's 7.1 row shows only the start-up glitch. The parking-lot entry said 5.1 only; the spec
  widened it.
- The decisive part: on the wiring cli-timer actually uses (`Player::connect_new` then `append`,
  `src/sound.rs:337-338`, `src/app.rs:225-228`), the output is rotated by two channels from sample
  512 onward whatever `FillChannels` reports. The proposed formula changed 0 frames at equal rates
  and made the resampling case **worse** (70 % to 100 % wrong in steady state: today's 32 768-sample
  cuts, 32 768 = 2 mod 6, add 2 to the rotation at each cut, so one chunk in three is
  accidentally right; aligned cuts freeze the rotation in place).
- The proposed formula is correct, and fully effective, only when `FillChannels` is the source the
  mixer's `UniformSourceIterator` reads directly (last row: 0 wrong frames with resampling). So it is
  half of a real fix whose other half the spec does not know exists.
- The re-query question: yes, a span reported shorter than the real one is re-queried.
  `UniformSourceIterator::next` re-bootstraps when its `Take` runs out (`uniform.rs:76-89`) and asks
  again. But rodio's symphonia decoder reports the whole packet length, not what remains
  (`decoder/symphonia.rs:163-165`, `buffer.len()`), and `SamplesBuffer` reports `data.len()`
  (`buffer.rs:76-82`), so the re-query from mid-packet gets the full packet again and the next chunk
  crosses the packet boundary. That is harmless here only because each chunk is a multiple of 6 and a
  single file never changes channels or rate. rodio's own contract (`source/mod.rs:181-182`) says
  `current_span_len` is "the total span size, not the remaining samples", while `Buffered` and
  `UniformSourceIterator` treat it as remaining. The proposed value is neither, which the spec
  should say rather than discover.
- The proof offered ("a unit test ... the reported length is a multiple of 6 and at most 32 768";
  "Nobody can hear the difference on a stereo output; the test is the proof") proves arithmetic. It
  passes today's broken output and tomorrow's identical one.

What to build instead: either drop item 1 from this batch and file a bead for "stereo `-s` on 5.1 is
rotated by two channels (rodio 0.23 Player bootstrap)", or, if surround matters, make item 1 the
wiring fix: feed the mixer a source whose first reported channel count is already the widened one
(for example keep `Player` for `stop`/`sleep_until_end` semantics but verify an ordering that
works, or add `fit_channels(..)` to the mixer directly with a stop flag), plus the proposed formula,
with a test that pulls samples from `rodio::mixer::mixer(6, 48_000)` and checks frame alignment
(no device needed; the probe shows this is cheap and deterministic). Note the append-then-add
ordering fixes equal rates but not resampling, so the mixer-direct route is the one the table
supports.

## Objection 2 (a, a simpler design with most of the value): ship one item, not three

- Item 1, as specified, has measured value zero (objection 1).
- Item 2: the hyphen-leading full-screen label **already works today** in two spellings. Probe
  against `TuiArgs` as it is now (clap 4.6.7, `args_conflicts_with_subcommands = true`,
  subcommands present): `-t 1m -l=-standup` parses to label `-standup`, and so does
  `-t 1m -l-standup`. Only the space-separated `-l -standup` fails ("a value is required for
  '-l <LABEL>'"). The spec rejects the `-s` change because "a sound file named `-x.mp3` can be
  passed as `-s ./-x.mp3`, and nobody has hit it"; the same two facts hold for `-l` (a working
  spelling exists; the parking-lot entry says "if it ever matters" and no one has hit it). By the
  spec's own rule, item 2 is also a non-goal. The 80 % design is one line in `--help`/README:
  "a label starting with `-`: write `-l=-standup`".
- Item 2 also adds a silent behaviour the spec's trade section misses: with the change,
  `cli-timer -t 1m -l --help` (and `-l -h`) starts a timer labelled `--help` instead of erroring
  (probe: today "a value is required", after: `Ok(label: "--help")`). The spec's "It still fails,
  with a different message" is true for its one example (`-l -t 1m` gives "the subcommand '1m'
  cannot be used with '-l <LABEL>'", which names a subcommand the user never typed) but not as a
  general claim.
- That leaves item 3, which is the only item with a user-visible effect, and it has its own
  problem (objection 3).

## Objection 3 (c, the collision in six months): versioned archive and folder names vs. "put it on your PATH"

- The parking-lot entry this comes from is conditional: "add the tag **if people keep several**".
  Nothing establishes that anyone keeps several; the last recorded facts are 0 stars and 0 forks
  (2026-10-23 red team), and the developer installs with `cargo install --path .` (NEXT.md "Your
  install points at this folder").
- The spec renames not only the archive but the **folder inside it**
  (`cli-timer-0.2.3-x86_64-pc-windows-msvc/`). README says "Unpack it and put `cli-timer` somewhere
  on your `PATH`". Today, unpacking a new release over the old one refreshes the binary in place and
  a PATH entry pointing at the unpacked folder stays right. After the change every release unpacks
  into a new folder, and a PATH entry made for the previous one silently keeps running the old
  binary. That collides with the plugin coupling in the parking lot: the skill ships with the
  crate version and expects a matching binary, so a stale binary on PATH is exactly the "skill that
  expects a different binary" failure the 2026-10-23 red team worried about, now produced by every
  release instead of by an install-order race.
- It also removes the one stable download URL GitHub offers,
  `releases/latest/download/<asset name>`, which needs an unchanging asset name. The skill tells an
  agent to "download a binary for the platform from https://github.com/ProgDroid/cli-timer/releases"
  (`plugin/skills/timer/SKILL.md:37`); an agent or script that ever automates that wants a fixed
  name. Whether any script outside this repo already uses one is UNKNOWN: a grep of the dotfiles
  tree was stopped before it finished.
- Cheaper variant with the stated benefit: keep the inner folder name versionless (or keep the
  archive name versionless and put the version only in the release title, which GitHub already
  shows). If versioned names are wanted, ripgrep-style, say in README that each release unpacks to
  a new folder and what to point PATH at.

Secondary 6-month risk, recorded rather than ranked: item 1 hard-codes rodio's private constant
32 768 from two internals (`buffered.rs:112`, `uniform.rs:55`) with no test that fails if rodio
changes it. The parking lot already expects rodio's "next big release" to force another pass over
`src/sound.rs`, and rodio 0.23 ships an `experimental` `FixedSource` trait with no span length at all
(`src/fixed_source.rs`). A mirror of an internal constant is the first thing that goes stale there.

## Fixed check: facts set side by side, and what a reader infers

| # | Sentence (paraphrased) | Facts paired | Reader infers | Verdict |
|---|---|---|---|---|
| 1 | §1: "`Buffered` (`buffered.rs:112`) takes at most 32 768 samples per span. 32 768 is not a whole number of 6- or 8-channel frames, so a long widened span ... can be cut" | Buffered caps at 32 768 (true) + 32 768 not whole 6-frames (true; false for 8) | Buffered is what cuts `FillChannels`' widened output | **Unsupported.** Buffered is upstream of `FillChannels`; the cut is in `UniformSourceIterator`. Kept as objection 1. |
| 2 | §1: "Today it reports `len / from * to`" + the cap | Formula (true) + a cap (true) | Today's mis-aligned surround output is caused by this formula, so changing it aligns the output | **Unsupported, and measured false**: same wrong frames at equal rates, more with resampling. Objection 1. |
| 3 | §1: "Nobody can hear the difference on a stereo output; the test is the proof." | No stereo-output effect (true: `fit_channels` returns the source unwrapped) + a unit test of the reported length | The change is proven correct where it matters (surround) | **Unsupported.** The test measures a number, not frames reaching the device. |
| 4 | §2: "Add `allow_hyphen_values` ... `cli-timer -t 1m -l -standup` then works." | The change (true) + the example then parses (true, probed) | A hyphen-leading label cannot be given today | **Unsupported.** `-l=-standup` and `-l-standup` work today. Objection 2. |
| 5 | §2 "Picked over": "`-s ./-x.mp3` works, and nobody has hit it" | A working spelling exists + nobody hit it | `-l` differs from `-s` on these points, which is why it gets the change | **Unsupported.** Both facts hold for `-l` too. Objection 2. |
| 6 | §2 "Known trade": "`-l -t 1m` ... still fails, with a different message" | Fails before + fails after | No input changes from error to silent success | **Unsupported.** `-l --help` / `-l -h` now succeed as labels. |
| 7 | §3: "Nothing links to an exact archive name (README and the skill point at the Releases page), so the rename breaks no link." | README and skill name no archive (true: `git grep` hits only `release.yml`, control term present) | Nothing depends on the archive or folder name | **Unsupported.** "No link in this repo" is not "no dependency": the inner folder name and `latest/download` URLs are uses outside the repo. Objection 3. |
| 8 | §3 "Proof": "a dry run shows four archives with the version in their names" + "upload-artifact names may stay `cli-timer-<target>`" | Dry run uploads 4 artifacts + their names stay versionless | Looking at the run page proves the rename | **Unsupported as written.** The Actions page lists artifact names (versionless); the archive file names are visible only after downloading an artifact. Say "download one artifact and list it". |
| 9 | Intent: "three `docs/parking-lot.md` entries, picked by the developer at session start" + §2 "Picked over also changing `-s`" | Developer picked the batch + spec chose `-l` only | The developer picked this exact scope | **Unsupported by the files.** NEXT.md's feedback log (uncommitted diff) records the pick as "hyphen-leading full-screen `-l`/`-s` values". Either the narrowing was agreed later (then NEXT.md is stale) or it was not (then ask). |
| 10 | Parking lot (inherited by the spec): "Only matters when the device's sample rate differs ... and a stereo `-s` file has packets over 5 461 frames" | Two conditions | Below them, surround output is right | **Unsupported.** Probe: wrong at equal rates and with 4 000-frame spans. NEXT.md's "a rare mid-frame cut is possible on surround outputs with unusual files" understates it the same way; both need correcting with whatever is decided. |

Residuals decomposed: the two control rows and the 6-channel passthrough row sum exactly (shown
above). The 70 % steady-state figure for today's formula with resampling: chunks are 32 768 input
samples (5 461.3 frames, about 5 944 output frames at 44.1 to 48 kHz); each cut adds 2 to the
rotation, so offsets cycle 2, 4, 0 and one chunk in three is right. The 1 s window holds about 6.6
chunks, of which 2 are offset-0: 2 x 5 944 = 11 888 right frames predicted, 39 486 - 27 539 =
11 947 observed; the 59-frame remainder is chunk-edge frames that partially overlap the window.

Displacement: flag 1 and 2 are objection 1, flags 4 and 5 are objection 2, and flag 7 is
objection 3, so no flagged implicature is left over to displace one of them.

## Smaller notes

- The spec cites `buffered.rs:112`; if item 1 survives, cite `uniform.rs:55` and `player.rs:73-76`.
- The new parse error for `-l -t 1m` names a "subcommand '1m'"; if item 2 survives, a test should
  pin that message or the trade note should quote it.
- `verify` greps the first `^version` line of `Cargo.toml`; if item 3 reuses that in `build`, the
  dry run (where `verify` is skipped) is the only place a mismatch between the two copies would show.
  A shared step or output is the safer of the builder's two options.
