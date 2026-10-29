//! Behaviour-tree interpreter for the game's monster trees, loaded from `_data/recovered/ai/trees.json` (from
//! the recovered graphs of `BT_Monster_Default` and its subtrees). The tree structure, decorators, services
//! and their numbers are the game's; this module runs them with Unreal's engine semantics for the standard
//! nodes (CONFIRMED: selector, sequence, run-behaviour, wait, finish-with-result, blackboard / force-success
//! / time-limit / loop / conditional-loop / tag-cooldown decorators, observer aborts Self and LowerPriority,
//! service intervals with random deviation). The game's own nodes (`BTT_*`, `BTD_*`, `BTS_*`) are handed to a
//! [`Host`] (`crate::ai`). A SimpleParallel runs its main task and, beside it, its background branch over and
//! over (Unreal's semantics): the main task's end aborts the background (`FinishMode` 0) or waits for its
//! current run (1). Simplifications (HYPOTHESIS): a lower-priority observer re-enters its slot when the whole
//! slot passes; a ValueChange observer behaves like a ResultChange one; a background branch ticks after the
//! main task.

use std::collections::{BTreeMap, HashMap};

use bevy_math::Vec3;
use serde_json::Value;

use crate::{DT, SimId, SimRng};

pub type NodeId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Selector,
    Sequence,
    Parallel,
    /// `BTTask_RunBehavior`: runs another tree's root as its only child.
    Subtree,
    Task,
    Decorator,
    Service,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub node: NodeId,
    pub decorators: Vec<NodeId>,
    /// `DecoratorOps` (operation, number): 1 Test, 2 And, 3 Or, 4 Not; empty = all decorators ANDed.
    pub ops: Vec<(u8, u32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub class: String,
    pub name: String,
    pub kind: Kind,
    /// Tree the node belongs to and its export index there (for docs and debugging).
    pub tree: String,
    pub index: u32,
    pub props: Value,
    pub children: Vec<Slot>,
    pub services: Vec<NodeId>,
}

impl Node {
    pub fn key(&self, name: &str) -> Option<&str> {
        self.props.get(name).and_then(Value::as_str)
    }
    pub fn f32(&self, name: &str) -> Option<f32> {
        self.props.get(name).and_then(Value::as_f64).map(|v| v as f32)
    }
    pub fn int(&self, name: &str) -> Option<i64> {
        self.props.get(name).and_then(Value::as_i64)
    }
    pub fn flag(&self, name: &str) -> bool {
        self.props.get(name).and_then(Value::as_bool).unwrap_or(false)
    }
    /// `FlowAbortMode`: 0 None, 1 LowerPriority, 2 Self, 3 Both.
    pub fn abort_mode(&self) -> i64 {
        self.int("FlowAbortMode").unwrap_or(0)
    }
}

/// All trees in one arena; `RunBehavior` nodes point at the root of the tree they run.
#[derive(Debug, Clone, Default)]
pub struct Trees {
    pub nodes: Vec<Node>,
    pub roots: HashMap<String, NodeId>,
}

impl Trees {
    pub fn parse(json: &Value) -> anyhow::Result<Self> {
        let mut t = Trees::default();
        let obj = json.as_object().ok_or_else(|| anyhow::anyhow!("trees: not an object"))?;
        // Pass 1: arena ids per (tree, index).
        let mut ids: HashMap<(String, u32), NodeId> = HashMap::new();
        for (name, tree) in obj {
            for k in tree["nodes"].as_object().into_iter().flatten().map(|(k, _)| k) {
                let idx: u32 = k.parse()?;
                ids.insert((name.clone(), idx), (ids.len()) as NodeId);
            }
        }
        t.nodes = vec![Node { class: String::new(), name: String::new(), kind: Kind::Task, tree: String::new(), index: 0, props: Value::Null, children: Vec::new(), services: Vec::new() }; ids.len()];
        for (name, tree) in obj {
            let id_of = |i: u64| ids.get(&(name.clone(), i as u32)).copied().ok_or_else(|| anyhow::anyhow!("{name}: missing node {i}"));
            t.roots.insert(name.clone(), id_of(tree["root"].as_u64().unwrap_or(0))?);
            for (k, n) in tree["nodes"].as_object().into_iter().flatten() {
                let class = n["class"].as_str().unwrap_or("").to_string();
                let kind = match class.as_str() {
                    "BTComposite_Selector" => Kind::Selector,
                    "BTComposite_Sequence" => Kind::Sequence,
                    "BTComposite_SimpleParallel" => Kind::Parallel,
                    "BTTask_RunBehavior" => Kind::Subtree,
                    c if c.starts_with("BTDecorator_") || c.starts_with("BTD_") => Kind::Decorator,
                    c if c.starts_with("BTService_") || c.starts_with("BTS_") => Kind::Service,
                    _ => Kind::Task,
                };
                let mut children = Vec::new();
                for c in n["children"].as_array().into_iter().flatten() {
                    let decorators = c["decorators"].as_array().into_iter().flatten().map(|d| id_of(d.as_u64().unwrap_or(0))).collect::<anyhow::Result<_>>()?;
                    let ops = c["ops"].as_array().into_iter().flatten().map(|o| (o[0].as_u64().unwrap_or(1) as u8, o[1].as_u64().unwrap_or(0) as u32)).collect();
                    children.push(Slot { node: id_of(c["node"].as_u64().unwrap_or(0))?, decorators, ops });
                }
                let services = n["services"].as_array().into_iter().flatten().map(|s| id_of(s.as_u64().unwrap_or(0))).collect::<anyhow::Result<_>>()?;
                let id = id_of(k.parse()?)?;
                t.nodes[id as usize] = Node { class, name: n["name"].as_str().unwrap_or("").to_string(), kind, tree: name.clone(), index: k.parse()?, props: n["props"].clone(), children, services };
            }
        }
        // Link run-behaviour nodes to the trees they run.
        for i in 0..t.nodes.len() {
            if t.nodes[i].kind == Kind::Subtree {
                let target = t.nodes[i].key("BehaviorAsset").unwrap_or("").to_string();
                let root = *t.roots.get(&target).ok_or_else(|| anyhow::anyhow!("RunBehavior to unknown tree {target}"))?;
                t.nodes[i].children = vec![Slot { node: root, decorators: Vec::new(), ops: Vec::new() }];
            }
        }
        Ok(t)
    }

    /// Loads `_data/recovered/ai/trees.json` ([`db_formats::data_root`]).
    pub fn load() -> anyhow::Result<Self> {
        let path = Some(db_formats::data_path("recovered/ai/trees.json")).filter(|p| p.is_file()).ok_or_else(|| anyhow::anyhow!("no _data/recovered/ai/trees.json (tools/recon/bt_trees.py)"))?;
        Self::parse(&serde_json::from_slice(&std::fs::read(path)?)?)
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }
}

/// A blackboard value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BbValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Vector(Vec3),
    Actor(SimId),
}

/// The blackboard (`BB_Monster_Default` keys by name).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Blackboard(pub BTreeMap<String, BbValue>);

impl Blackboard {
    pub fn set(&mut self, key: &str, v: BbValue) {
        self.0.insert(key.to_string(), v);
    }
    pub fn clear(&mut self, key: &str) {
        self.0.remove(key);
    }
    pub fn get(&self, key: &str) -> Option<BbValue> {
        self.0.get(key).copied()
    }
    /// Unreal's "Is Set": true bool, valid object, any vector, non-zero number.
    pub fn is_set(&self, key: &str) -> bool {
        match self.get(key) {
            Some(BbValue::Bool(b)) => b,
            Some(BbValue::Int(i)) => i != 0,
            Some(BbValue::Float(f)) => f != 0.0,
            Some(BbValue::Vector(_)) | Some(BbValue::Actor(_)) => true,
            None => false,
        }
    }
    pub fn bool(&self, key: &str) -> bool {
        matches!(self.get(key), Some(BbValue::Bool(true)))
    }
    pub fn float(&self, key: &str) -> Option<f32> {
        match self.get(key) {
            Some(BbValue::Float(f)) => Some(f),
            Some(BbValue::Int(i)) => Some(i as f32),
            _ => None,
        }
    }
    pub fn int(&self, key: &str) -> i64 {
        match self.get(key) {
            Some(BbValue::Int(i)) => i,
            _ => 0,
        }
    }
    pub fn vector(&self, key: &str) -> Option<Vec3> {
        match self.get(key) {
            Some(BbValue::Vector(v)) => Some(v),
            _ => None,
        }
    }
    pub fn actor(&self, key: &str) -> Option<SimId> {
        match self.get(key) {
            Some(BbValue::Actor(a)) => Some(a),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Succeeded,
    Failed,
}

/// Per-task scratch the host may use while its task runs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TaskMem {
    pub started: u32,
    pub until: u32,
    pub last_pos: Vec3,
    pub stuck: u32,
    pub index: usize,
    pub points: Vec<Vec3>,
    /// A move's nav route: its floor points, the next one, the goal it was planned for and when.
    pub route: Vec<Vec3>,
    pub route_index: usize,
    pub route_goal: Vec3,
    pub route_at: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceEvent {
    Activate,
    Tick,
    Deactivate,
}

/// The game's own nodes, implemented by the sim.
pub trait Host {
    fn rng(&mut self) -> &mut SimRng;
    /// A `BTD_*` decorator's raw condition (before `bInverseCondition`).
    fn condition(&mut self, node: &Node, bb: &Blackboard) -> bool;
    fn start(&mut self, node: &Node, bb: &mut Blackboard, mem: &mut TaskMem, now: u32) -> Status;
    fn tick(&mut self, node: &Node, bb: &mut Blackboard, mem: &mut TaskMem, now: u32) -> Status;
    fn abort(&mut self, node: &Node, bb: &mut Blackboard);
    fn service(&mut self, node: &Node, bb: &mut Blackboard, event: ServiceEvent, now: u32);
}

#[derive(Debug, Clone, PartialEq)]
struct Frame {
    node: NodeId,
    /// Index of the running child slot, or `usize::MAX` before the first.
    child: usize,
    /// Tick the running child slot was entered (time limits).
    slot_since: u32,
    /// The running slot's Self-abort observers' last results: a slot with composite decorator logic is
    /// re-evaluated only when one of them changes, as Unreal's observers notify on a result change.
    observed: Option<Vec<bool>>,
}

/// A SimpleParallel's background branch: its own execution, sharing the blackboard and tag cooldowns.
#[derive(Debug, Clone, PartialEq)]
struct Background {
    /// Stack depth of the parallel's frame, and the parallel.
    depth: usize,
    parallel: NodeId,
    state: Box<BtState>,
    /// The main task's result, while a waiting parallel (`FinishMode` 1) lets the background's run end.
    main_done: Option<Status>,
}

/// One monster's tree execution: the active path, the running task, active services, blackboard.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BtState {
    stack: Vec<Frame>,
    backgrounds: Vec<Background>,
    task: Option<(NodeId, TaskMem)>,
    services: Vec<(NodeId, u32)>,
    /// When each service that has been active is next due, kept while it is inactive (Unreal keeps a
    /// service's timer in its node memory unless `bRestartTimerOnEachActivation`).
    service_due: BTreeMap<NodeId, u32>,
    cooldowns: BTreeMap<String, u32>,
    now: u32,
    pub bb: Blackboard,
}

const MAX_STEPS: usize = 256;

fn ticks(s: f32) -> u32 {
    (s.max(0.0) / DT).round() as u32
}

impl BtState {
    /// The running task, if any.
    pub fn task(&self) -> Option<NodeId> {
        self.task.as_ref().map(|t| t.0)
    }
    /// Trees on the active path (outermost first).
    pub fn active_trees<'a>(&self, trees: &'a Trees) -> Vec<&'a str> {
        let mut out: Vec<&str> = Vec::new();
        for f in &self.stack {
            let t = trees.node(f.node).tree.as_str();
            if out.last() != Some(&t) {
                out.push(t);
            }
        }
        out
    }

    /// Stops everything (death, reset).
    pub fn reset(&mut self, trees: &Trees, host: &mut impl Host, now: u32) {
        self.unwind(trees, host, 0, now);
    }

    /// One tick: observer aborts and time limits, services, the running task, then as many instant
    /// steps as the tree takes until a task is running again.
    pub fn tick(&mut self, trees: &Trees, root: NodeId, host: &mut impl Host, now: u32) {
        self.now = now;
        if self.stack.is_empty() && self.task.is_none() {
            self.enter_root(trees, root, host, now);
        } else {
            self.check_aborts(trees, host, now);
        }
        self.tick_services(trees, host, now);
        let mut steps = 0;
        let mut pending: Option<Status> = None;
        if let Some((id, mut mem)) = self.task.take() {
            let node = trees.node(id);
            let st = self.task_tick(node, host, &mut mem, now);
            if st == Status::Running {
                self.task = Some((id, mem));
            } else {
                self.deactivate_services_of(trees, id, host, now);
                pending = Some(st);
            }
        }
        while let Some(result) = pending.take() {
            steps += 1;
            if steps > MAX_STEPS {
                break;
            }
            pending = self.deliver(trees, result, host, now);
        }
        self.tick_backgrounds(trees, host, now);
        // A finished root starts over next tick (Unreal restarts it at once; a tick apart keeps instant
        // loops bounded).
    }

    fn enter_root(&mut self, trees: &Trees, root: NodeId, host: &mut impl Host, now: u32) {
        if let Some(r) = self.enter_node(trees, root, host, now) {
            let mut pending = Some(r);
            let mut steps = 0;
            while let Some(res) = pending.take() {
                steps += 1;
                if steps > MAX_STEPS {
                    break;
                }
                pending = self.deliver(trees, res, host, now);
            }
        }
    }

    /// Enters a node: composites push a frame and pick their first child; tasks start. Returns a result
    /// if the node finished at once (to be delivered to the parent frame).
    fn enter_node(&mut self, trees: &Trees, id: NodeId, host: &mut impl Host, now: u32) -> Option<Status> {
        let node = trees.node(id);
        self.activate_services(trees, node, host, now);
        match node.kind {
            Kind::Selector | Kind::Sequence | Kind::Parallel | Kind::Subtree => {
                self.stack.push(Frame { node: id, child: usize::MAX, slot_since: now, observed: None });
                if node.kind == Kind::Parallel && node.children.len() > 1 {
                    self.backgrounds.push(Background { depth: self.stack.len() - 1, parallel: id, state: Box::default(), main_done: None });
                }
                self.advance(trees, host, now)
            }
            _ => {
                let mut mem = TaskMem { started: now, ..Default::default() };
                let st = self.task_start(node, host, &mut mem, now);
                if st == Status::Running {
                    self.task = Some((id, mem));
                    None
                } else {
                    self.deactivate_services_of(trees, id, host, now);
                    Some(st)
                }
            }
        }
    }

    /// The top frame moves on to its next child slot whose decorators pass; returns the frame's own
    /// result if it has none left.
    fn advance(&mut self, trees: &Trees, host: &mut impl Host, now: u32) -> Option<Status> {
        let top = self.stack.last()?.clone();
        let node = trees.node(top.node);
        // A parallel's slot here is its main task (its background runs beside it); a subtree runs its root.
        let last = match node.kind {
            Kind::Parallel | Kind::Subtree => 1.min(node.children.len()),
            _ => node.children.len(),
        };
        let mut i = if top.child == usize::MAX { 0 } else { top.child + 1 };
        while i < last {
            let slot = &node.children[i];
            if self.slot_passes(trees, slot, host) {
                if let Some(f) = self.stack.last_mut() {
                    f.child = i;
                    f.slot_since = now;
                    f.observed = None;
                }
                return self.enter_node(trees, slot.node, host, now);
            }
            // A skipped slot counts as failed (a sequence stops, a selector tries the next), unless it has
            // a force-success decorator, which Unreal also notifies for a slot whose conditions fail
            // (the trees rely on it: optional steps such as `BT_Ability`'s Stare).
            let forced = slot.decorators.iter().any(|&d| trees.node(d).class == "BTDecorator_ForceSuccess");
            match (node.kind, forced) {
                (Kind::Sequence, false) => return self.pop_frame(trees, Status::Failed, host, now),
                (Kind::Selector, true) => return self.pop_frame(trees, Status::Succeeded, host, now),
                _ => {}
            }
            i += 1;
        }
        let result = match node.kind {
            Kind::Sequence => Status::Succeeded,
            _ => Status::Failed,
        };
        self.pop_frame(trees, result, host, now)
    }

    fn pop_frame(&mut self, trees: &Trees, result: Status, host: &mut impl Host, now: u32) -> Option<Status> {
        let f = self.stack.pop()?;
        self.deactivate_services_of(trees, f.node, host, now);
        self.drop_backgrounds(trees, host, now);
        Some(result)
    }

    /// Aborts the background branches whose parallel left the stack.
    fn drop_backgrounds(&mut self, trees: &Trees, host: &mut impl Host, now: u32) {
        while let Some(i) = self.backgrounds.iter().position(|b| b.depth >= self.stack.len()) {
            let mut b = self.backgrounds.remove(i);
            std::mem::swap(&mut self.bb, &mut b.state.bb);
            b.state.reset(trees, host, now);
            std::mem::swap(&mut self.bb, &mut b.state.bb);
        }
    }

    /// Each running parallel's background branch: (re)started while its slot's decorators pass, ticked with the
    /// shared blackboard; a waiting parallel whose main task ended finishes when the background's run does.
    fn tick_backgrounds(&mut self, trees: &Trees, host: &mut impl Host, now: u32) {
        let mut i = 0;
        while i < self.backgrounds.len() {
            let (depth, parallel) = (self.backgrounds[i].depth, self.backgrounds[i].parallel);
            if self.stack.get(depth).map(|f| f.node) != Some(parallel) {
                i += 1;
                continue;
            }
            let Some(slot) = trees.node(parallel).children.get(1) else {
                i += 1;
                continue;
            };
            let idle = |b: &Background| b.state.stack.is_empty() && b.state.task.is_none();
            let (was_idle, main_done) = (idle(&self.backgrounds[i]), self.backgrounds[i].main_done);
            if !(was_idle && (main_done.is_some() || !self.slot_passes(trees, slot, host))) {
                let mut b = self.backgrounds.remove(i);
                std::mem::swap(&mut self.bb, &mut b.state.bb);
                std::mem::swap(&mut self.cooldowns, &mut b.state.cooldowns);
                b.state.tick(trees, slot.node, host, now);
                std::mem::swap(&mut self.bb, &mut b.state.bb);
                std::mem::swap(&mut self.cooldowns, &mut b.state.cooldowns);
                self.backgrounds.insert(i, b);
            }
            // The waiting parallel's background run is over: the parallel ends with its main task's result.
            if let Some(result) = main_done
                && idle(&self.backgrounds[i])
            {
                self.unwind_above(trees, host, depth + 1, now);
                let mut pending = self.pop_frame(trees, result, host, now);
                while let Some(res) = pending.take() {
                    pending = self.deliver(trees, res, host, now);
                }
                return;
            }
            i += 1;
        }
    }

    /// Drops the frames above `depth` (not the running task).
    fn unwind_above(&mut self, trees: &Trees, host: &mut impl Host, depth: usize, now: u32) {
        while self.stack.len() > depth {
            if let Some(f) = self.stack.pop() {
                self.deactivate_services_of(trees, f.node, host, now);
            }
        }
        self.drop_backgrounds(trees, host, now);
    }

    /// A child of the top frame finished with `result`: apply its slot decorators (loops, force success,
    /// tag cooldowns), then let the composite decide.
    fn deliver(&mut self, trees: &Trees, mut result: Status, host: &mut impl Host, now: u32) -> Option<Status> {
        let top = self.stack.last()?.clone();
        let node = trees.node(top.node);
        let Some(slot) = node.children.get(top.child) else { return self.pop_frame(trees, result, host, now) };
        // Loops re-run the child.
        for &d in &slot.decorators {
            let dn = trees.node(d);
            let again = match dn.class.as_str() {
                "BTDecorator_ConditionalLoop" => self.blackboard_test(dn),
                "BTDecorator_Loop" => result == Status::Succeeded && dn.flag("bInfiniteLoop"),
                _ => false,
            };
            if again {
                if let Some(f) = self.stack.last_mut() {
                    f.slot_since = now;
                    f.observed = None;
                }
                return self.enter_node(trees, slot.node, host, now);
            }
        }
        for &d in &slot.decorators {
            let dn = trees.node(d);
            match dn.class.as_str() {
                "BTDecorator_ForceSuccess" => result = Status::Succeeded,
                "BTDecorator_SetTagCooldown" => {
                    let tag = dn.key("CooldownTag").unwrap_or("").to_string();
                    self.cooldowns.insert(tag, now + ticks(dn.f32("CooldownDuration").unwrap_or(5.0)));
                }
                _ => {}
            }
        }
        // A waiting parallel (`FinishMode` 1) whose background is mid-run lets that run end first.
        if node.kind == Kind::Parallel
            && node.int("FinishMode") == Some(1)
            && let Some(b) = self.backgrounds.iter_mut().find(|b| b.depth == self.stack.len() - 1 && b.parallel == top.node)
            && (!b.state.stack.is_empty() || b.state.task.is_some())
        {
            b.main_done = Some(result);
            return None;
        }
        match (node.kind, result) {
            (Kind::Selector, Status::Succeeded) | (Kind::Sequence, Status::Failed) | (Kind::Parallel | Kind::Subtree, _) => self.pop_frame(trees, result, host, now),
            _ => self.advance(trees, host, now),
        }
    }

    /// All decorators of a slot (with `DecoratorOps` logic when present).
    fn slot_passes(&mut self, trees: &Trees, slot: &Slot, host: &mut impl Host) -> bool {
        let values: Vec<bool> = slot.decorators.iter().map(|&d| self.decorator(trees.node(d), host)).collect();
        if slot.ops.is_empty() {
            return values.iter().all(|v| *v);
        }
        let mut at = 0;
        eval_ops(&slot.ops, &mut at, &values)
    }

    /// One decorator's condition (loops, force success, time limits and cooldown setters always pass).
    fn decorator(&mut self, d: &Node, host: &mut impl Host) -> bool {
        let raw = match d.class.as_str() {
            "BTDecorator_Blackboard" => self.blackboard_test(d),
            "BTDecorator_TagCooldown" => self.cooldowns.get(d.key("CooldownTag").unwrap_or("")).is_none_or(|&until| until <= self.now),
            c if c.starts_with("BTDecorator_") => true,
            _ => host.condition(d, &self.bb),
        };
        raw != d.flag("bInverseCondition")
    }

    /// Blackboard key test: OperationType 0 Is Set, 1 Is Not Set (bool and object keys here).
    fn blackboard_test(&self, d: &Node) -> bool {
        let set = self.bb.is_set(d.key("BlackboardKey").unwrap_or(""));
        if d.int("OperationType").unwrap_or(0) == 1 { !set } else { set }
    }

    /// Observer aborts (Self and Both on the running slots, LowerPriority and Both on higher-priority
    /// selector slots) and time limits, outermost first.
    fn check_aborts(&mut self, trees: &Trees, host: &mut impl Host, now: u32) {
        for depth in 0..self.stack.len() {
            let f = self.stack[depth].clone();
            let node = trees.node(f.node);
            let Some(slot) = node.children.get(f.child) else { continue };
            // Higher-priority siblings that now pass.
            if node.kind == Kind::Selector {
                for j in 0..f.child {
                    let s = &node.children[j];
                    if s.decorators.iter().any(|&d| matches!(trees.node(d).abort_mode(), 1 | 3)) && self.slot_passes(trees, s, host) {
                        self.unwind(trees, host, depth + 1, now);
                        if let Some(top) = self.stack.last_mut() {
                            top.child = j;
                            top.slot_since = now;
                            top.observed = None;
                        }
                        if let Some(r) = self.enter_node(trees, s.node, host, now) {
                            let mut pending = Some(r);
                            while let Some(res) = pending.take() {
                                pending = self.deliver(trees, res, host, now);
                            }
                        }
                        return;
                    }
                }
            }
            // The running slot's own observers and time limits: each observer re-checks its own condition; a slot
            // with composite decorator logic re-evaluates all of it, but only when an observer's result changed
            // (`BT_Ability`'s attack branch observes `AbilityTarget` through an always-true Or, so its
            // `InAbilityRange` test alone never aborts a running attack).
            let observers: Vec<NodeId> = slot.decorators.iter().copied().filter(|&d| matches!(trees.node(d).abort_mode(), 2 | 3)).collect();
            let results: Vec<bool> = observers.iter().map(|&d| self.decorator(trees.node(d), host)).collect();
            let changed = self.stack[depth].observed.as_ref().is_some_and(|o| *o != results);
            self.stack[depth].observed = Some(results.clone());
            let self_abort = !observers.is_empty() && if slot.ops.is_empty() { results.iter().any(|r| !r) } else { changed && !self.slot_passes(trees, slot, host) };
            let timed_out = slot.decorators.iter().any(|&d| {
                let dn = trees.node(d);
                let limit = match dn.class.as_str() {
                    "BTDecorator_TimeLimit" => Some(dn.f32("TimeLimit").unwrap_or(5.0)),
                    "BTDecorator_TimeLimitEx" => self.bb.float(dn.key("BlackboardKey").unwrap_or("")).or(dn.f32("TimeLimit")).or(Some(5.0)),
                    _ => None,
                };
                limit.is_some_and(|l| now >= f.slot_since + ticks(l))
            });
            if self_abort || timed_out {
                self.unwind(trees, host, depth + 1, now);
                // The aborted child counts as failed (force-success still applies).
                let mut pending = self.deliver(trees, Status::Failed, host, now);
                while let Some(res) = pending.take() {
                    pending = self.deliver(trees, res, host, now);
                }
                return;
            }
        }
    }

    /// Drops every frame at `depth` and below and the running task (aborting it).
    fn unwind(&mut self, trees: &Trees, host: &mut impl Host, depth: usize, now: u32) {
        if let Some((id, _)) = self.task.take() {
            host.abort(trees.node(id), &mut self.bb);
            self.deactivate_services_of(trees, id, host, now);
        }
        while self.stack.len() > depth {
            if let Some(f) = self.stack.pop() {
                self.deactivate_services_of(trees, f.node, host, now);
            }
        }
        self.drop_backgrounds(trees, host, now);
    }

    fn activate_services(&mut self, trees: &Trees, node: &Node, host: &mut impl Host, now: u32) {
        for &s in &node.services {
            let sn = trees.node(s);
            host.service(sn, &mut self.bb, ServiceEvent::Activate, now);
            // First due one interval after activation. A later activation keeps the timer (Unreal keeps it
            // in the node memory unless `bRestartTimerOnEachActivation`). The engine counts it down only
            // while the service is active; here it runs on regardless (HYPOTHESIS): a branch left and
            // re-entered every tick, such as a chase already at its goal, would otherwise never tick its
            // services (`BTS_CheckInAbilityRange` never ran for a monster that woke up next to its target).
            let fresh = |this: &Self, host: &mut _| now + this.interval(sn, host);
            let next = if sn.flag("bCallTickOnSearchStart") {
                host.service(sn, &mut self.bb, ServiceEvent::Tick, now);
                fresh(self, host)
            } else if sn.flag("bRestartTimerOnEachActivation") {
                fresh(self, host)
            } else {
                match self.service_due.get(&s).copied() {
                    // Already due (it was due while inactive): it ticks now, on entry, since the branch may
                    // end before the next service pass.
                    Some(due) if due <= now => {
                        host.service(sn, &mut self.bb, ServiceEvent::Tick, now);
                        fresh(self, host)
                    }
                    Some(due) => due,
                    None => fresh(self, host),
                }
            };
            self.services.push((s, next));
        }
    }

    fn deactivate_services_of(&mut self, trees: &Trees, id: NodeId, host: &mut impl Host, now: u32) {
        for &s in &trees.node(id).services {
            if let Some(i) = self.services.iter().position(|x| x.0 == s) {
                let (_, due) = self.services.remove(i);
                self.service_due.insert(s, due);
                host.service(trees.node(s), &mut self.bb, ServiceEvent::Deactivate, now);
            }
        }
    }

    /// Service interval with its random deviation (engine defaults 0.5 ± 0.1 s), at least one tick.
    fn interval(&self, sn: &Node, host: &mut impl Host) -> u32 {
        let base = sn.f32("Interval").unwrap_or(0.5);
        let dev = sn.f32("RandomDeviation").unwrap_or(0.1);
        let s = base + dev * (host.rng().unit_f32() * 2.0 - 1.0);
        ticks(s).max(1)
    }

    fn tick_services(&mut self, trees: &Trees, host: &mut impl Host, now: u32) {
        for i in 0..self.services.len() {
            let (s, next) = self.services[i];
            if now >= next {
                let sn = trees.node(s);
                host.service(sn, &mut self.bb, ServiceEvent::Tick, now);
                let iv = self.interval(sn, host);
                if let Some(x) = self.services.get_mut(i) {
                    x.1 = now + iv;
                }
            }
        }
    }

    /// Standard engine tasks here, the game's through the host.
    fn task_start(&mut self, node: &Node, host: &mut impl Host, mem: &mut TaskMem, now: u32) -> Status {
        match node.class.as_str() {
            "BTTask_Wait" => {
                let dev = node.f32("RandomDeviation").unwrap_or(0.0);
                let s = node.f32("WaitTime").unwrap_or(5.0) + dev * (host.rng().unit_f32() * 2.0 - 1.0);
                mem.until = now + ticks(s);
                if mem.until <= now { Status::Succeeded } else { Status::Running }
            }
            "BTTask_WaitBlackboardTime" => {
                mem.until = now + ticks(self.bb.float(node.key("BlackboardKey").unwrap_or("")).unwrap_or(0.0));
                if mem.until <= now { Status::Succeeded } else { Status::Running }
            }
            "BTTask_FinishWithResult" => match node.int("Result").unwrap_or(0) {
                1 => Status::Failed,
                _ => Status::Succeeded,
            },
            "BTTask_SetTagCooldown" => {
                self.cooldowns.insert(node.key("CooldownTag").unwrap_or("").to_string(), now + ticks(node.f32("CooldownDuration").unwrap_or(5.0)));
                Status::Succeeded
            }
            _ => host.start(node, &mut self.bb, mem, now),
        }
    }

    fn task_tick(&mut self, node: &Node, host: &mut impl Host, mem: &mut TaskMem, now: u32) -> Status {
        match node.class.as_str() {
            "BTTask_Wait" | "BTTask_WaitBlackboardTime" => {
                if now >= mem.until { Status::Succeeded } else { Status::Running }
            }
            _ => host.tick(node, &mut self.bb, mem, now),
        }
    }
}

/// Evaluates `DecoratorOps` (prefix notation) from position `at`.
fn eval_ops(ops: &[(u8, u32)], at: &mut usize, values: &[bool]) -> bool {
    let Some(&(op, n)) = ops.get(*at) else { return true };
    *at += 1;
    match op {
        1 => values.get(n as usize).copied().unwrap_or(true),
        2 => (0..n).map(|_| eval_ops(ops, at, values)).fold(true, |a, b| a & b),
        3 => (0..n).map(|_| eval_ops(ops, at, values)).fold(false, |a, b| a | b),
        4 => !eval_ops(ops, at, values),
        _ => true,
    }
}
