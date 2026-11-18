//! Randomized parity between an agent and the open editor. Each round builds a settled random tree (plain frames,
//! auto-layout rows and columns, groups), sends the agent one to four ops as a single batch through the real tool
//! path, replays the patch it recorded in the editor, and checks that the two documents are equal node by node, that
//! undoing the replayed step gives back the editor's document from before the patch, and that redoing it gives the
//! agent's document again. The generator is a fixed-seed xorshift so a failure names the seed and the batch.

use crate::app::App;
use crate::group_delete_layout_tests::{frame_of, rect_in};
use crate::group_live_patch_parity_tests::{assert_same_documents_with, pair};
use designos_doc::live_patch::LivePatch;
use designos_doc::{AutoLayout, Document, FrameProps, LayoutAlign, LayoutDirection, NodeId, NodeKind, Rect, Sizing};
use serde_json::{json, Value};

/// Rounds per run: enough to meet every shape, a few seconds in CI. `PARITY_ROUNDS` and `PARITY_SEED` override.
const ROUNDS: u64 = 120;
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn between(&mut self, lo: u32, hi: u32) -> f32 {
        (lo + (self.next() % u64::from(hi - lo + 1)) as u32) as f32
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Frame,
    Group,
    Layout,
}

struct Container {
    id: NodeId,
    shape: Shape,
    children: usize,
}

/// The tree the generator built, as the batch generator reads it: containers with their child counts and the leaves.
struct Tree {
    containers: Vec<Container>,
    leaves: Vec<(NodeId, usize)>,
}

fn layout_kind(rng: &mut Rng) -> NodeKind {
    let direction = if rng.below(2) == 0 { LayoutDirection::Vertical } else { LayoutDirection::Horizontal };
    let axis = |rng: &mut Rng| if rng.below(2) == 0 { Sizing::Hug } else { Sizing::Fixed };
    let layout = AutoLayout { direction, primary_sizing: axis(rng), counter_sizing: axis(rng), primary_align: LayoutAlign::Start, ..AutoLayout::default() };
    NodeKind::Frame(FrameProps { auto_layout: Some(layout), ..FrameProps::default() })
}

fn grow(doc: &mut Document, rng: &mut Rng, tree: &mut Tree, parent: usize, depth: usize) {
    let count = 2 + rng.below(4);
    for _ in 0..count {
        let make_container = depth < 4 && rng.below(10) < 4;
        let (pid, in_layout) = (tree.containers[parent].id, tree.containers[parent].shape == Shape::Layout);
        let at = if in_layout { (0.0, 0.0) } else { (rng.between(0, 120), rng.between(0, 120)) };
        tree.containers[parent].children += 1;
        if make_container {
            let (shape, kind) = match rng.below(3) {
                0 => (Shape::Frame, NodeKind::Frame(FrameProps::default())),
                1 => (Shape::Group, NodeKind::Group),
                _ => (Shape::Layout, layout_kind(rng)),
            };
            let (w, h) = (rng.between(160, 400), rng.between(160, 400));
            let id = frame_of(doc, pid, Rect::new(at.0, at.1, w, h), kind);
            tree.containers.push(Container { id, shape, children: 0 });
            let index = tree.containers.len() - 1;
            grow(doc, rng, tree, index, depth + 1);
        } else {
            let id = rect_in(doc, pid, Rect::new(at.0, at.1, rng.between(10, 120), rng.between(10, 120)));
            tree.leaves.push((id, parent));
        }
    }
}

/// Solves every auto-layout frame and fits every group, repeatedly, until the document stops changing, so the round
/// starts where the solver would leave it. `false` when it does not settle.
fn settle(doc: &mut Document, page: NodeId, groups: &[NodeId]) -> bool {
    for _ in 0..8 {
        let before = doc.clone();
        doc.relayout_subtree(page);
        doc.flush_pending_relayout();
        for &g in groups.iter().rev() {
            designos_doc::fit_group_to_children(doc, g, |_, _| {});
        }
        if doc.nodes.iter().all(|(id, n)| before.get(*id) == Some(n)) {
            return true;
        }
    }
    false
}

/// Whether a `move` may share a batch with other ops. `origin/main` already trips the batch tool's trial-versus-commit
/// assertion on some of those batches (see [`MAIN_MIXED_MOVE_SEEDS`]), so the default rounds send a move alone.
#[derive(Clone, Copy, PartialEq)]
enum Moves {
    Alone,
    Mixed,
}

/// One to four ops that keep every group non-empty and name only layers that still exist.
fn batch(rng: &mut Rng, tree: &mut Tree, moves: Moves) -> Vec<Value> {
    let mut ops = Vec::new();
    for _ in 0..1 + rng.below(4) {
        if moves == Moves::Alone && ops.iter().any(|op: &Value| op["op"] == "move") {
            break;
        }
        match rng.below(4) {
            0 => {
                let c = rng.below(tree.containers.len());
                tree.containers[c].children += 1;
                ops.push(json!({ "op": "create", "parent": tree.containers[c].id.0, "kind": "rectangle",
                    "x": rng.between(0, 120), "y": rng.between(0, 120), "width": rng.between(10, 120), "height": rng.between(10, 120) }));
            }
            1 if !tree.leaves.is_empty() => {
                let (id, _) = tree.leaves[rng.below(tree.leaves.len())];
                let field = if rng.below(2) == 0 { "width" } else { "height" };
                ops.push(json!({ "op": "update", "node": id.0, field: rng.between(10, 160) }));
            }
            2 if !tree.leaves.is_empty() => {
                let at = rng.below(tree.leaves.len());
                let (id, c) = tree.leaves[at];
                if tree.containers[c].children > 1 {
                    tree.containers[c].children -= 1;
                    tree.leaves.remove(at);
                    ops.push(json!({ "op": "delete", "node": id.0 }));
                }
            }
            3 if !tree.leaves.is_empty() && (moves == Moves::Mixed || ops.is_empty()) => {
                let at = rng.below(tree.leaves.len());
                let (id, from) = tree.leaves[at];
                let to = rng.below(tree.containers.len());
                if to != from && tree.containers[from].children > 1 {
                    tree.containers[from].children -= 1;
                    tree.containers[to].children += 1;
                    tree.leaves[at].1 = to;
                    ops.push(json!({ "op": "move", "node": id.0, "parent": tree.containers[to].id.0 }));
                }
            }
            _ => {}
        }
    }
    ops
}

fn snapshot(app: &App) -> Vec<(NodeId, Value)> {
    app.doc.nodes.values().map(|n| (n.id, serde_json::to_value(n).unwrap())).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Outcome {
    Ran,
    Unsettled,
    NoOps,
    ToolRefused,
    UndoInexact,
    RedoInexact,
}

/// What a round checks after the editor replayed the patch.
#[derive(Clone, Copy, PartialEq)]
enum Check {
    /// The editor's document equals the agent's, node by node.
    Equality,
    /// The editor against itself: undo of the replayed step gives back its document from before the patch, and redo
    /// gives back its own document from after it. A mismatch fails the round.
    UndoExact,
    /// The same, counted instead of failed, with the first seeds that differ printed.
    UndoCount,
}

/// Names the seed and the batch when a round panics anywhere, in the agent's tool included.
struct Round(String);

impl Drop for Round {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("round that panicked: {}", self.0);
        }
    }
}

fn run_round(seed: u64, check: Check, moves: Moves) -> Outcome {
    let mut rng = Rng(seed);
    let mut tree = Tree { containers: Vec::new(), leaves: Vec::new() };
    let (mut engine, mut app, settled) = pair(|doc, page| {
        let root = frame_of(doc, page, Rect::new(0.0, 0.0, 600.0, 600.0), layout_kind(&mut rng));
        tree.containers.push(Container { id: root, shape: Shape::Layout, children: 0 });
        grow(doc, &mut rng, &mut tree, 0, 1);
        let groups: Vec<NodeId> = tree.containers.iter().filter(|c| c.shape == Shape::Group).map(|c| c.id).collect();
        settle(doc, page, &groups)
    });
    // A round whose start the solver cannot settle is skipped: parity is claimed from a settled document.
    if !settled {
        return Outcome::Unsettled;
    }
    let ops = batch(&mut rng, &mut tree, moves);
    if ops.is_empty() {
        return Outcome::NoOps;
    }
    let before_patch = snapshot(&app);
    let args = json!({ "ops": ops });
    let context = format!("seed {seed:#x}, batch {args}");
    let _named = Round(context.clone());
    if engine.call("designos_apply_batch", args.clone()).is_err() {
        return Outcome::ToolRefused;
    }
    let live = engine.take_live_ops();
    let layout = engine.take_live_layout();
    if live.is_empty() {
        return Outcome::NoOps;
    }
    let mut patch: LivePatch = serde_json::from_str(&serde_json::to_string(&LivePatch::new(live).with_layout(layout)).unwrap()).unwrap();
    // Test switch: send the patch without the layout the agent solved, as a build that predates the field would.
    if std::env::var_os("PARITY_STRIP_LAYOUT").is_some() {
        patch.layout = None;
    }
    crate::live_patch_apply::apply_live_patches(&mut app, &[patch]);

    if check == Check::Equality {
        assert_same_documents_with(&engine, &app, &context);
        return Outcome::Ran;
    }
    let after_patch = snapshot(&app);
    app.undo();
    let undone = snapshot(&app) == before_patch;
    app.redo();
    let redone = snapshot(&app) == after_patch;
    if check == Check::UndoExact {
        assert!(undone, "undo of the replayed step restores the editor's document: {context}");
        assert!(redone, "redo of the replayed step restores the editor's document from after the patch: {context}");
    }
    match (undone, redone) {
        (true, true) => Outcome::Ran,
        (false, _) => {
            eprintln!("UNDO INEXACT {context}");
            Outcome::UndoInexact
        }
        (true, false) => {
            eprintln!("REDO INEXACT {context}");
            Outcome::RedoInexact
        }
    }
}

fn rounds_and_seed() -> (u64, u64) {
    let rounds = std::env::var("PARITY_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(ROUNDS);
    let seed = std::env::var("PARITY_SEED").ok().and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(SEED);
    (rounds, seed)
}

/// `PARITY_MIXED_MOVES=1` lets a move share a batch with other ops, to replay a seed from before moves were sent alone.
fn moves_from_env() -> Moves {
    if std::env::var_os("PARITY_MIXED_MOVES").is_some() {
        Moves::Mixed
    } else {
        Moves::Alone
    }
}

fn run_rounds(check: Check) -> std::collections::BTreeMap<Outcome, u64> {
    // `PARITY_ROUND_SEED` replays one round by the seed a failure printed.
    if let Some(one) = std::env::var("PARITY_ROUND_SEED").ok().and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok()) {
        return [(run_round(one, check, moves_from_env()), 1)].into();
    }
    let (rounds, seed) = rounds_and_seed();
    let mut tally = std::collections::BTreeMap::new();
    for round in 0..rounds {
        let mut mix = Rng(seed ^ round.wrapping_mul(0xD6E8_FEB8_6659_FD93).wrapping_add(1));
        *tally.entry(run_round(mix.next() | 1, check, moves_from_env())).or_insert(0u64) += 1;
    }
    eprintln!("PARITY TALLY {tally:?}");
    let ran = tally.get(&Outcome::Ran).copied().unwrap_or(0);
    let counted: u64 = tally.iter().filter(|(o, _)| matches!(o, Outcome::UndoInexact | Outcome::RedoInexact)).map(|(_, n)| n).sum();
    assert!((ran + counted) * 2 >= rounds, "at least half the rounds ran a patch end to end: {tally:?}");
    tally
}

/// The editor's document equals the agent's after the patch, for every round the agent's tool accepts. A panic in the
/// agent's tool (its trial and committed documents differing, for one) fails the test and names the seed.
#[test]
fn random_agent_batches_leave_the_editor_with_the_agents_document() {
    run_rounds(Check::Equality);
}

/// Undo of the replayed step gives back the editor's document from before the patch, and redo its own document from
/// after it. Does not hold yet, and does not hold without the carried layout either: layout the editor writes after
/// the commit is outside the step, so undo of an agent step solves again from the restored tree. `--ignored` shows
/// the first seed.
#[test]
#[ignore = "undo of a replayed agent step is not exact yet: the solve after the commit is outside the step"]
fn random_agent_batches_replayed_step_undoes_and_redoes_exactly() {
    run_rounds(Check::UndoExact);
}

/// The undo and redo rates, counted: run with `--ignored --nocapture`, and with `PARITY_STRIP_LAYOUT=1` to send the
/// patches without the carried layout.
#[test]
#[ignore = "a probe that prints the undo and redo rates"]
fn random_agent_batches_undo_and_redo_rates() {
    run_rounds(Check::UndoCount);
}

/// Round seeds whose batch mixes a `move` with other ops and trips the batch tool's own assertion that the committed
/// document equals its trial copy. The same five seeds do it on `origin/main` (a trial that solves per op against a
/// commit that solves once), so this predates the group refit. They are generated with `Moves::Mixed`.
const MAIN_MIXED_MOVE_SEEDS: [u64; 5] = [0x489a_7883_85b1_67c9, 0x8cee_48e8_ee40_2637, 0x2081_4e8e_d795_7365, 0x827d_5994_3d25_7e0f, 0xeeb2_c6db_2bb6_0c81];

/// Red until the batch tool's trial and commit agree on those batches; run with `--ignored`.
#[test]
#[ignore = "the batch tool's trial and committed documents differ on these batches, as they do on origin/main"]
fn batches_mixing_a_move_with_other_ops_commit_what_their_trial_holds() {
    for seed in MAIN_MIXED_MOVE_SEEDS {
        run_round(seed, Check::Equality, Moves::Mixed);
    }
}
