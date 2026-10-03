//! A minimal deterministic relational graph simulator.
//!
//! Everything is integer arithmetic with a fixed iteration order, and the only
//! source of randomness is a SplitMix64 generator seeded from the job seed.
//! Any compliant node replaying the same engine, ruleset, initial conditions
//! and seed therefore reproduces the same final state hash and metrics.
//!
//! The metrics are development instruments for exercising the protocol. They
//! are not validated measures of emergence, and a simulation result shows at
//! most that a rule *can* produce a structure, not that nature does.

// Vertex indices address several parallel arrays at once; index loops read
// more clearly here than zipped iterators.
#![allow(clippy::needless_range_loop)]

use eel_core::simulation::*;
use eel_core::{ArtifactId, CanonicalSerialize, ContentHash, Hash, IdentityId, SimulationResultId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const ENGINE_VERSION: &str = "relational-graph-engine/0.1.0";

#[derive(Debug, Error)]
pub enum SimError {
    #[error("ruleset hash does not match the job")]
    RulesetMismatch,
    #[error("initial conditions hash does not match the job")]
    InitialConditionsMismatch,
    #[error("engine version `{0}` is not supported by this node")]
    EngineVersion(String),
    #[error("seed {0} is outside the job's seed range")]
    SeedOutOfRange(u64),
    #[error("invalid parameters: {0}")]
    InvalidParameters(&'static str),
    #[error(transparent)]
    Core(#[from] eel_core::CoreError),
}

pub type Result<T> = std::result::Result<T, SimError>;

/// Hash identifying this engine implementation.
pub fn engine_hash() -> ContentHash {
    ContentHash(Hash::digest(ENGINE_VERSION.as_bytes()))
}

pub fn ruleset_hash(r: &Ruleset) -> Result<ContentHash> {
    Ok(ContentHash(r.canonical_hash()?))
}

pub fn initial_conditions_hash(ic: &InitialConditions) -> Result<ContentHash> {
    Ok(ContentHash(ic.canonical_hash()?))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vertex {
    pub id: u64,
    pub state: i64,
    pub memory: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub source: u64,
    pub target: u64,
    pub weight: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimGraph {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
}

/// SplitMix64: tiny, fast and fully specified.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform-ish integer in [-bound, bound] (modulo bias is accepted; it is deterministic).
    pub fn symmetric(&mut self, bound: i64) -> i64 {
        if bound <= 0 {
            return 0;
        }
        let span = (2 * bound + 1) as u64;
        (self.next_u64() % span) as i64 - bound
    }
    /// True with probability permille/1000.
    pub fn chance(&mut self, permille: i64) -> bool {
        (self.next_u64() % 1000) < permille.clamp(0, 1000) as u64
    }
}

const INIT_SALT: u64 = 0x4545_4c2d_494e_4954; // "EEL-INIT"
const STEP_SALT: u64 = 0x4545_4c2d_5354_4550; // "EEL-STEP"

/// Builds the initial graph from the conditions and seed.
pub fn initialize_graph(ic: &InitialConditions, seed: u64) -> SimGraph {
    let mut rng = Rng::new(seed ^ INIT_SALT);
    let n = ic.vertices as u64;
    let vertices = (0..n)
        .map(|id| Vertex {
            id,
            state: rng.symmetric(ic.initial_state_bound),
            memory: vec![],
        })
        .collect();
    let mut edges = vec![];
    for source in 0..n {
        for target in 0..n {
            if source != target && rng.chance(ic.edge_permille) {
                edges.push(Edge {
                    source,
                    target,
                    weight: rng.symmetric(ic.weight_bound),
                });
            }
        }
    }
    SimGraph { vertices, edges }
}

fn validate(r: &Ruleset, ic: &InitialConditions) -> Result<()> {
    if r.coupling_divisor <= 0 {
        return Err(SimError::InvalidParameters(
            "coupling_divisor must be positive",
        ));
    }
    if r.state_bound <= 0 || r.persistence_threshold < 0 {
        return Err(SimError::InvalidParameters("bounds must be positive"));
    }
    if r.memory_length == 0 || r.memory_length > 1024 {
        return Err(SimError::InvalidParameters(
            "memory_length must be 1..=1024",
        ));
    }
    if ic.vertices == 0 || ic.vertices > 4096 {
        return Err(SimError::InvalidParameters("vertices must be 1..=4096"));
    }
    Ok(())
}

/// Union-find over vertex indices.
struct Dsu(Vec<usize>);
impl Dsu {
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.0[r] != r {
            r = self.0[r];
        }
        let mut c = x;
        while self.0[c] != r {
            let n = self.0[c];
            self.0[c] = r;
            c = n;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.0[hi] = lo;
        }
    }
}

/// Connected components (ignoring edge direction) of persistent vertices,
/// keeping components of at least two vertices, in ascending order.
fn persistent_structures(g: &SimGraph, persistent: &[bool]) -> Vec<Vec<usize>> {
    let n = g.vertices.len();
    let mut dsu = Dsu((0..n).collect());
    for e in &g.edges {
        let (s, t) = (e.source as usize, e.target as usize);
        if persistent[s] && persistent[t] {
            dsu.union(s, t);
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for v in 0..n {
        if persistent[v] {
            let root = dsu.find(v);
            groups.entry(root).or_default().push(v);
        }
    }
    groups.into_values().filter(|c| c.len() >= 2).collect()
}

/// Number of distinct structure pairs bridged by a single intermediate vertex
/// (structure → v → structure). A crude proxy for structures composing into
/// higher-order structures.
fn composed_pairs(g: &SimGraph, structures: &[Vec<usize>]) -> i64 {
    let n = g.vertices.len();
    let mut owner = vec![usize::MAX; n];
    for (i, s) in structures.iter().enumerate() {
        for &v in s {
            owner[v] = i;
        }
    }
    let mut touching: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];
    for e in &g.edges {
        let (s, t) = (e.source as usize, e.target as usize);
        if owner[s] != usize::MAX && owner[t] == usize::MAX {
            touching[t].insert(owner[s]);
        }
        if owner[t] != usize::MAX && owner[s] == usize::MAX {
            touching[s].insert(owner[t]);
        }
    }
    let mut pairs = BTreeSet::new();
    for set in touching {
        let items: Vec<usize> = set.into_iter().collect();
        for i in 0..items.len() {
            for j in i + 1..items.len() {
                pairs.insert((items[i], items[j]));
            }
        }
    }
    pairs.len() as i64
}

/// Output of one deterministic run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunOutput {
    pub seed: u64,
    pub initial_state_hash: ContentHash,
    pub final_state_hash: ContentHash,
    pub metrics: Vec<MetricValue>,
    pub final_graph: SimGraph,
}

/// Runs the simulation loop:
///
/// ```text
/// initialize_graph(seed)
/// for each timestep:
///     apply_local_interactions
///     generate_deterministic_perturbations
///     apply_constraints
///     update_vertex_states
///     detect_persistent_structures
///     record_metrics
/// ```
pub fn run(
    ruleset: &Ruleset,
    ic: &InitialConditions,
    seed: u64,
    steps: u64,
    wanted: &[SimulationMetric],
) -> Result<RunOutput> {
    validate(ruleset, ic)?;
    let mut g = initialize_graph(ic, seed);
    let initial_state_hash = ContentHash(g.canonical_hash()?);
    let n = g.vertices.len();
    let mut rng = Rng::new(seed ^ STEP_SALT);
    let mem_len = ruleset.memory_length as usize;

    let mut persistent = vec![false; n];
    let mut run_len = vec![0i64; n];
    let mut lifespan = 0i64;
    let mut steps_with_structure = 0i64;
    let mut perturbed_persistent = 0i64;
    let mut survived = 0i64;
    let mut seen_sets: BTreeSet<Hash> = BTreeSet::new();
    let mut recurrence = 0i64;
    let mut sign_snapshots: BTreeSet<Hash> = BTreeSet::new();
    let mut max_composed = 0i64;

    for _ in 0..steps {
        // apply_local_interactions
        let mut incoming = vec![0i128; n];
        for e in &g.edges {
            incoming[e.target as usize] +=
                e.weight as i128 * g.vertices[e.source as usize].state as i128;
        }
        let mut next: Vec<i128> = (0..n)
            .map(|v| {
                g.vertices[v].state as i128 * ruleset.self_retention_permille as i128 / 1000
                    + incoming[v] / ruleset.coupling_divisor as i128
            })
            .collect();
        // generate_deterministic_perturbations
        let mut perturbed = vec![false; n];
        for v in 0..n {
            if rng.chance(ruleset.perturbation_permille) {
                next[v] += rng.symmetric(ruleset.perturbation_magnitude) as i128;
                perturbed[v] = true;
            }
        }
        // apply_constraints
        let bound = ruleset.state_bound as i128;
        // update_vertex_states
        for v in 0..n {
            let s = next[v].clamp(-bound, bound) as i64;
            let vx = &mut g.vertices[v];
            vx.state = s;
            vx.memory.push(s);
            if vx.memory.len() > mem_len {
                vx.memory.remove(0);
            }
        }
        // detect_persistent_structures
        let was_persistent = persistent.clone();
        for v in 0..n {
            let m = &g.vertices[v].memory;
            persistent[v] = m.len() == mem_len
                && m.iter()
                    .all(|x| x.abs() >= ruleset.persistence_threshold && *x != 0)
                && (m.iter().all(|x| *x > 0) || m.iter().all(|x| *x < 0));
        }
        let structures = persistent_structures(&g, &persistent);
        // record_metrics
        for v in 0..n {
            run_len[v] = if persistent[v] { run_len[v] + 1 } else { 0 };
            lifespan = lifespan.max(run_len[v]);
            if was_persistent[v] && perturbed[v] {
                perturbed_persistent += 1;
                if persistent[v] {
                    survived += 1;
                }
            }
        }
        if !structures.is_empty() {
            steps_with_structure += 1;
            let set_hash = structures.canonical_hash()?;
            if !seen_sets.insert(set_hash) {
                recurrence += 1;
            }
        }
        let signs: Vec<i8> = g.vertices.iter().map(|v| v.state.signum() as i8).collect();
        sign_snapshots.insert(signs.canonical_hash()?);
        max_composed = max_composed.max(composed_pairs(&g, &structures));
    }

    let steps_i = steps.max(1) as i64;
    let value = |m: SimulationMetric| -> i64 {
        match m {
            SimulationMetric::Lifespan => lifespan,
            SimulationMetric::StructuralPersistence => steps_with_structure * 1000 / steps_i,
            SimulationMetric::PerturbationResistance => {
                if perturbed_persistent == 0 {
                    -1
                } else {
                    survived * 1000 / perturbed_persistent
                }
            }
            SimulationMetric::Recurrence => recurrence,
            SimulationMetric::CompressionRatio => {
                1000 - (sign_snapshots.len() as i64 * 1000 / steps_i).min(1000)
            }
            SimulationMetric::RecursiveComposability => max_composed,
        }
    };
    let metrics = wanted
        .iter()
        .map(|&m| MetricValue {
            metric: m,
            value: value(m),
        })
        .collect();
    let final_state_hash = ContentHash(g.canonical_hash()?);
    Ok(RunOutput {
        seed,
        initial_state_hash,
        final_state_hash,
        metrics,
        final_graph: g,
    })
}

/// Abstraction over engines whose work may later count as useful work.
///
/// TODO(EEL-FUTURE): PoEUW validator reward integration
/// TODO(EEL-FUTURE): zkVM verification
/// TODO(EEL-FUTURE): probabilistic challenge verification
pub trait UsefulWorkEngine {
    /// Runs the job's first seed.
    fn execute(&self, job: &SimulationJob) -> Result<SimulationResult>;
    /// Replays a result and reports whether every hash and metric matches.
    fn verify(&self, result: &SimulationResult) -> Result<bool>;
}

/// The MVP engine, bound to one job's registered ruleset and initial conditions.
pub struct RelationalGraphEngine {
    pub job: SimulationJob,
    pub ruleset: Ruleset,
    pub initial_conditions: InitialConditions,
    pub worker: IdentityId,
}

/// The artifact written for every result: enough to audit the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultArtifact {
    pub engine_version: String,
    pub job_id: eel_core::SimulationJobId,
    pub seed: u64,
    pub steps: u64,
    pub metrics: Vec<MetricValue>,
    pub final_graph: SimGraph,
}

impl RelationalGraphEngine {
    pub fn new(
        job: SimulationJob,
        ruleset: Ruleset,
        initial_conditions: InitialConditions,
        worker: IdentityId,
    ) -> Result<Self> {
        if job.engine_version != ENGINE_VERSION {
            return Err(SimError::EngineVersion(job.engine_version.clone()));
        }
        if ruleset_hash(&ruleset)? != job.ruleset_hash {
            return Err(SimError::RulesetMismatch);
        }
        if initial_conditions_hash(&initial_conditions)? != job.initial_conditions_hash {
            return Err(SimError::InitialConditionsMismatch);
        }
        Ok(RelationalGraphEngine {
            job,
            ruleset,
            initial_conditions,
            worker,
        })
    }

    /// Runs one seed and returns the result and its artifact bytes.
    pub fn execute_seed(&self, seed: u64) -> Result<(SimulationResult, Vec<u8>)> {
        if seed < self.job.seed_start || seed > self.job.seed_end {
            return Err(SimError::SeedOutOfRange(seed));
        }
        let out = run(
            &self.ruleset,
            &self.initial_conditions,
            seed,
            self.job.steps,
            &self.job.metrics,
        )?;
        let artifact = ResultArtifact {
            engine_version: ENGINE_VERSION.to_string(),
            job_id: self.job.job_id.clone(),
            seed,
            steps: self.job.steps,
            metrics: out.metrics.clone(),
            final_graph: out.final_graph,
        };
        let artifact_bytes = artifact.canonical_bytes()?;
        let mut result = SimulationResult {
            result_id: SimulationResultId(Hash::ZERO),
            job_id: self.job.job_id.clone(),
            worker: self.worker,
            seed,
            engine_hash: engine_hash(),
            ruleset_hash: self.job.ruleset_hash,
            initial_state_hash: out.initial_state_hash,
            final_state_hash: out.final_state_hash,
            metrics: out.metrics,
            result_artifact: ArtifactId(Hash::digest(&artifact_bytes)),
        };
        result.result_id = result.compute_id()?;
        Ok((result, artifact_bytes))
    }
}

impl UsefulWorkEngine for RelationalGraphEngine {
    fn execute(&self, job: &SimulationJob) -> Result<SimulationResult> {
        Ok(self.execute_seed(job.seed_start)?.0)
    }

    fn verify(&self, result: &SimulationResult) -> Result<bool> {
        if result.job_id != self.job.job_id
            || result.engine_hash != engine_hash()
            || result.ruleset_hash != self.job.ruleset_hash
            || result.result_id != result.compute_id()?
        {
            return Ok(false);
        }
        if result.seed < self.job.seed_start || result.seed > self.job.seed_end {
            return Ok(false);
        }
        let (replayed, _) = self.execute_seed(result.seed)?;
        // The worker is not part of the computation; compare everything else.
        Ok(replayed.initial_state_hash == result.initial_state_hash
            && replayed.final_state_hash == result.final_state_hash
            && replayed.metrics == result.metrics
            && replayed.result_artifact == result.result_artifact)
    }
}

/// Builds a job whose hashes match the given ruleset and initial conditions.
#[allow(clippy::too_many_arguments)]
pub fn make_job(
    job_id: eel_core::SimulationJobId,
    linked_claims: Vec<eel_core::ClaimId>,
    ruleset: &Ruleset,
    ic: &InitialConditions,
    seed_start: u64,
    seed_end: u64,
    steps: u64,
    metrics: Vec<SimulationMetric>,
) -> Result<SimulationJob> {
    Ok(SimulationJob {
        job_id,
        linked_claims,
        engine_version: ENGINE_VERSION.to_string(),
        initial_conditions_hash: initial_conditions_hash(ic)?,
        ruleset_hash: ruleset_hash(ruleset)?,
        seed_start,
        seed_end,
        steps,
        metrics,
    })
}

pub fn all_metrics() -> Vec<SimulationMetric> {
    use SimulationMetric::*;
    vec![
        Lifespan,
        StructuralPersistence,
        PerturbationResistance,
        Recurrence,
        CompressionRatio,
        RecursiveComposability,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn ruleset() -> Ruleset {
        Ruleset {
            coupling_divisor: 8,
            self_retention_permille: 900,
            perturbation_permille: 50,
            perturbation_magnitude: 200,
            state_bound: 1000,
            persistence_threshold: 300,
            memory_length: 5,
        }
    }

    pub fn ic() -> InitialConditions {
        InitialConditions {
            vertices: 32,
            edge_permille: 120,
            weight_bound: 4,
            initial_state_bound: 1000,
        }
    }

    fn engine() -> RelationalGraphEngine {
        let job = make_job(
            "SIM-0001".parse().unwrap(),
            vec![],
            &ruleset(),
            &ic(),
            1,
            10,
            200,
            all_metrics(),
        )
        .unwrap();
        RelationalGraphEngine::new(job, ruleset(), ic(), IdentityId::default()).unwrap()
    }

    #[test]
    fn same_seed_same_final_hash() {
        let e = engine();
        let (a, _) = e.execute_seed(3).unwrap();
        let (b, _) = e.execute_seed(3).unwrap();
        assert_eq!(a, b);
        let (c, _) = e.execute_seed(4).unwrap();
        assert_ne!(a.final_state_hash, c.final_state_hash);
    }

    #[test]
    fn modified_ruleset_changes_hash() {
        let mut r = ruleset();
        r.coupling_divisor += 1;
        assert_ne!(ruleset_hash(&r).unwrap(), ruleset_hash(&ruleset()).unwrap());
    }

    #[test]
    fn fabricated_result_fails_replay() {
        let e = engine();
        let (mut r, _) = e.execute_seed(2).unwrap();
        assert!(e.verify(&r).unwrap());
        r.metrics[0].value += 1;
        r.result_id = r.compute_id().unwrap();
        assert!(!e.verify(&r).unwrap());
    }
}
