use crate::state::*;
use eel_core::event::*;
use eel_core::ontology::*;
use eel_core::research::*;
use eel_core::simulation::SimulationResult;
use eel_core::validation::*;
use eel_core::xchange::*;
use eel_core::*;
use eel_simulation::{RelationalGraphEngine, UsefulWorkEngine};
use eel_validation::{authorize_validator, canonicalization_blockers, ProposalFacts};
use std::collections::BTreeSet;
use thiserror::Error;

/// Upper bound on simulation steps, so that replay during projection stays cheap.
pub const MAX_SIMULATION_STEPS: u64 = 100_000;

#[derive(Debug, Error)]
pub enum ProjectionError {
    #[error("signature check failed: {0}")]
    Crypto(#[from] eel_crypto::CryptoError),
    #[error("author {0} is not a registered identity")]
    UnknownAuthor(IdentityId),
    #[error("branch `{0}` does not exist")]
    UnknownBranch(BranchId),
    #[error("validator: {0}")]
    Validator(#[from] eel_validation::ValidationError),
    #[error("simulation: {0}")]
    Simulation(#[from] eel_simulation::SimError),
    #[error("{0}")]
    Rejected(String),
    #[error(transparent)]
    Core(#[from] eel_core::CoreError),
}

type Result<T> = std::result::Result<T, ProjectionError>;

fn reject<T>(msg: impl Into<String>) -> Result<T> {
    Err(ProjectionError::Rejected(msg.into()))
}

macro_rules! ensure {
    ($cond:expr, $($fmt:tt)*) => {
        if !$cond {
            return reject(format!($($fmt)*));
        }
    };
}

/// Applies events to a projected state.
pub trait StateProjector {
    fn apply_event(&mut self, event: &EpistemicEvent) -> Result<()>;
}

impl StateProjector for OntologyState {
    /// Validates and applies one event atomically: if any rule rejects the
    /// event, the state is left unchanged.
    fn apply_event(&mut self, event: &EpistemicEvent) -> Result<()> {
        let mut next = self.clone();
        next.apply_inner(event)?;
        *self = next;
        Ok(())
    }
}

/// Context shared by all handlers.
struct Ctx {
    id: EventId,
    author: IdentityId,
    at: Timestamp,
    branch: BranchId,
}

impl Ctx {
    fn provenance(&self) -> Provenance {
        Provenance {
            author: self.author,
            event: self.id,
            timestamp: self.at,
        }
    }
}

impl OntologyState {
    fn apply_inner(&mut self, event: &EpistemicEvent) -> Result<()> {
        // Cryptographic verification against the author's registered key (or,
        // for self-registration, the key in the payload).
        let public_key = match &event.payload {
            EventPayload::IdentityRegister(r) => r.public_key.clone(),
            _ => self
                .identities
                .get(&event.author)
                .ok_or(ProjectionError::UnknownAuthor(event.author))?
                .public_key
                .clone(),
        };
        let id = eel_crypto::verify_event(event, &public_key)?;
        if !self.branches.contains_key(&event.branch) {
            return Err(ProjectionError::UnknownBranch(event.branch.clone()));
        }
        let ctx = Ctx {
            id,
            author: event.author,
            at: event.timestamp,
            branch: event.branch.clone(),
        };

        use EventPayload as P;
        match &event.payload {
            P::IdentityRegister(p) => self.identity_register(&ctx, p)?,
            P::ClaimCreate(p) => self.claim_create(&ctx, p)?,
            P::ClaimRevise(p) => self.claim_revise(&ctx, p)?,
            P::ClaimSupersede(p) => self.claim_supersede(&ctx, p)?,
            P::EvidenceAdd(p) => self.evidence_add(&ctx, p)?,
            P::EvidenceRetract(p) => self.evidence_retract(&ctx, p)?,
            P::ObjectionAdd(p) => self.objection_add(&ctx, p)?,
            P::ObjectionReply(p) => self.objection_reply(&ctx, p)?,
            P::ReplicationRegister(p) => self.replication_register(&ctx, p)?,
            P::ReplicationResult(p) => self.replication_result(&ctx, p)?,
            P::ValidationSubmit(p) => self.validation_submit(&ctx, p)?,
            P::BranchCreate(p) => self.branch_create(&ctx, p)?,
            P::BranchMergePropose(p) => self.merge_propose(&ctx, p)?,
            P::CanonicalizationPropose(p) => self.canon_propose(&ctx, p)?,
            P::CanonicalizationAccept(p) => self.canon_vote(&ctx, p, true)?,
            P::CanonicalizationReject(p) => self.canon_vote(&ctx, p, false)?,
            P::ArtifactRegister(p) => self.artifact_register(&ctx, p)?,
            P::SimulationDefine(p) => self.simulation_define(&ctx, p)?,
            P::SimulationResult(p) => self.simulation_result(&ctx, p)?,
            P::SimulationVerify(p) => self.simulation_verify(&ctx, p)?,
            P::ResearchRequestCreate(p) => self.research_request(&ctx, p)?,
            P::ResearchRequestSupport(p) => self.research_support(&ctx, p)?,
            P::ResearchResultSubmit(p) => self.research_result(&ctx, p)?,
            P::XchangeAssetRegister(p) => self.xchange_asset(&ctx, p)?,
        }

        for a in &event.artifact_refs {
            ensure!(
                self.artifacts.contains_key(a),
                "artifact {a} is not registered"
            );
        }
        let b = self.branches.get_mut(&ctx.branch).expect("checked above");
        b.head = Some(id);
        b.event_count += 1;
        self.events_applied += 1;
        Ok(())
    }

    fn bs(&self, branch: &BranchId) -> &BranchState {
        &self.branches[branch].state
    }

    fn bs_mut(&mut self, branch: &BranchId) -> &mut BranchState {
        &mut self.branches.get_mut(branch).expect("branch exists").state
    }

    /// Whether an object exists, resolving branch-scoped kinds on `branch`.
    fn object_exists(&self, branch: &BranchId, o: &ObjectId) -> bool {
        let b = self.bs(branch);
        let parse = |s: &str| s.parse::<Hash>().ok();
        match o.kind {
            ObjectKind::Claim => {
                o.id.parse()
                    .is_ok_and(|id: ClaimId| b.claims.contains_key(&id))
            }
            ObjectKind::Evidence => {
                o.id.parse()
                    .is_ok_and(|id: EvidenceId| b.evidence.contains_key(&id))
            }
            ObjectKind::Objection => {
                o.id.parse()
                    .is_ok_and(|id: ObjectionId| b.objections.contains_key(&id))
            }
            ObjectKind::Replication => {
                o.id.parse()
                    .is_ok_and(|id: ReplicationId| b.replications.contains_key(&id))
            }
            ObjectKind::Validation => {
                o.id.parse()
                    .is_ok_and(|id: ValidationId| b.validations.contains_key(&id))
            }
            ObjectKind::Artifact => {
                parse(&o.id).is_some_and(|h| self.artifacts.contains_key(&ArtifactId(h)))
            }
            ObjectKind::SimulationJob => {
                o.id.parse()
                    .is_ok_and(|id: SimulationJobId| self.simulation_jobs.contains_key(&id))
            }
            ObjectKind::SimulationResult => parse(&o.id)
                .is_some_and(|h| self.simulation_results.contains_key(&SimulationResultId(h))),
            ObjectKind::ResearchRequest => {
                o.id.parse()
                    .is_ok_and(|id: ResearchRequestId| self.research_requests.contains_key(&id))
            }
            ObjectKind::ResearchResult => {
                parse(&o.id).is_some_and(|h| self.research_results.contains_key(&EventId(h)))
            }
            ObjectKind::XchangeAsset => {
                o.id.parse()
                    .is_ok_and(|id: XchangeAssetId| self.xchange_assets.contains_key(&id))
            }
            ObjectKind::Branch => {
                o.id.parse()
                    .is_ok_and(|id: BranchId| self.branches.contains_key(&id))
            }
            ObjectKind::Identity => {
                parse(&o.id).is_some_and(|h| self.identities.contains_key(&IdentityId(h)))
            }
        }
    }

    fn claim_domains(&self, branch: &BranchId, claims: &[ClaimId]) -> BTreeSet<String> {
        let b = self.bs(branch);
        let mut out: BTreeSet<String> = claims
            .iter()
            .filter_map(|c| b.claims.get(c))
            .map(|c| c.current.domain.clone())
            .collect();
        if out.is_empty() {
            out.insert("general".into());
        }
        out
    }

    /// The domains a validator needs authority in to act on an object
    /// (decisions D-012).
    fn domains_of(&self, branch: &BranchId, o: &ObjectId) -> BTreeSet<String> {
        let b = self.bs(branch);
        let general = || BTreeSet::from(["general".to_string()]);
        match o.kind {
            ObjectKind::Claim => {
                o.id.parse::<ClaimId>()
                    .map(|c| self.claim_domains(branch, &[c]))
                    .unwrap_or_else(|_| general())
            }
            ObjectKind::Evidence => {
                o.id.parse::<EvidenceId>()
                    .ok()
                    .and_then(|e| b.evidence.get(&e))
                    .map(|e| self.claim_domains(branch, std::slice::from_ref(&e.evidence.claim_id)))
                    .unwrap_or_else(general)
            }
            ObjectKind::Objection => {
                o.id.parse::<ObjectionId>()
                    .ok()
                    .and_then(|x| b.objections.get(&x))
                    .map(|x| self.domains_of(branch, &x.objection.target))
                    .unwrap_or_else(general)
            }
            ObjectKind::Replication => {
                o.id.parse::<ReplicationId>()
                    .ok()
                    .and_then(|x| b.replications.get(&x))
                    .map(|x| self.domains_of(branch, &x.replication.target))
                    .unwrap_or_else(general)
            }
            ObjectKind::SimulationJob => {
                o.id.parse::<SimulationJobId>()
                    .ok()
                    .and_then(|j| self.simulation_jobs.get(&j))
                    .map(|j| self.claim_domains(branch, &j.definition.job.linked_claims))
                    .unwrap_or_else(general)
            }
            ObjectKind::SimulationResult => {
                o.id.parse::<Hash>()
                    .ok()
                    .and_then(|h| self.simulation_results.get(&SimulationResultId(h)))
                    .map(|r| {
                        self.domains_of(
                            branch,
                            &ObjectId::new(ObjectKind::SimulationJob, &r.result.job_id),
                        )
                    })
                    .unwrap_or_else(general)
            }
            ObjectKind::ResearchResult => {
                o.id.parse::<Hash>()
                    .ok()
                    .and_then(|h| self.research_results.get(&EventId(h)))
                    .map(|r| {
                        let mut claims = r.result.linked_claims.clone();
                        if let Some(req) = self.research_requests.get(&r.result.request_id) {
                            claims.extend(req.request.linked_claims.iter().cloned());
                        }
                        self.claim_domains(branch, &claims)
                    })
                    .unwrap_or_else(general)
            }
            ObjectKind::ResearchRequest => {
                o.id.parse::<ResearchRequestId>()
                    .ok()
                    .and_then(|r| self.research_requests.get(&r))
                    .map(|r| self.claim_domains(branch, &r.request.linked_claims))
                    .unwrap_or_else(general)
            }
            _ => general(),
        }
    }

    fn require_artifacts(&self, artifacts: &[ArtifactId]) -> Result<()> {
        for a in artifacts {
            ensure!(
                self.artifacts.contains_key(a),
                "artifact {a} is not registered"
            );
        }
        Ok(())
    }

    fn require_claims(&self, branch: &BranchId, claims: &[ClaimId]) -> Result<()> {
        let b = self.bs(branch);
        for c in claims {
            ensure!(
                b.claims.contains_key(c),
                "claim {c} does not exist on branch {branch}"
            );
        }
        Ok(())
    }

    fn add_edge(&mut self, ctx: &Ctx, source: ObjectId, relation: RelationType, target: ObjectId) {
        let edge = OntologyEdge {
            source,
            relation,
            target,
            originating_event: ctx.id,
        };
        self.bs_mut(&ctx.branch).edges.insert(edge);
    }

    // ---------------------------------------------------------------- identity

    fn identity_register(&mut self, ctx: &Ctx, p: &IdentityRegister) -> Result<()> {
        let key = eel_crypto::parse_public_key(&p.public_key)?;
        ensure!(
            eel_crypto::identity_id_for(&key.to_bytes()) == p.identity_id
                && p.identity_id == ctx.author,
            "identity id must be the hash of the public key and match the event author"
        );
        ensure!(
            !self.identities.contains_key(&p.identity_id),
            "identity {} is already registered",
            p.identity_id
        );
        self.identities.insert(
            p.identity_id,
            IdentityRecord {
                identity_id: p.identity_id,
                public_key: p.public_key.clone(),
                created_at: ctx.at,
                registered_by: Some(ctx.id),
            },
        );
        Ok(())
    }

    // ------------------------------------------------------------------ claims

    fn check_relations(&self, ctx: &Ctx, from: &ClaimId, relations: &[RelationSpec]) -> Result<()> {
        for r in relations {
            ensure!(
                !(r.target.kind == ObjectKind::Claim && r.target.id == from.as_str()),
                "a claim cannot relate to itself"
            );
            ensure!(
                self.object_exists(&ctx.branch, &r.target),
                "relation target {} does not exist",
                r.target
            );
            ensure!(
                !matches!(r.relation, RelationType::Supersedes),
                "use CLAIM_SUPERSEDE to supersede a claim"
            );
        }
        Ok(())
    }

    fn check_claim_body(&self, ctx: &Ctx, c: &Claim) -> Result<()> {
        ensure!(!c.title.trim().is_empty(), "claim title must not be empty");
        ensure!(
            !c.statement.trim().is_empty(),
            "claim statement must not be empty"
        );
        ensure!(
            !c.claim_type.is_empty(),
            "claim must have at least one claim type"
        );
        ensure!(
            !c.domain.trim().is_empty(),
            "claim domain must not be empty"
        );
        ensure!(
            !c.dependencies.contains(&c.claim_id),
            "a claim cannot depend on itself"
        );
        let unique: BTreeSet<_> = c.dependencies.iter().collect();
        ensure!(unique.len() == c.dependencies.len(), "duplicate dependency");
        self.require_claims(&ctx.branch, &c.dependencies)?;
        if let Some(od) = &c.ontological_defensibility {
            self.require_claims(&ctx.branch, &od.lower_order_dependencies)?;
        }
        Ok(())
    }

    fn record_claim_edges(&mut self, ctx: &Ctx, c: &Claim, relations: &[RelationSpec]) {
        let src = ObjectId::claim(&c.claim_id);
        for d in &c.dependencies {
            self.add_edge(
                ctx,
                src.clone(),
                RelationType::DependsOn,
                ObjectId::claim(d),
            );
        }
        for r in relations {
            self.add_edge(ctx, src.clone(), r.relation, r.target.clone());
        }
    }

    fn claim_create(&mut self, ctx: &Ctx, p: &ClaimCreate) -> Result<()> {
        let c = &p.claim;
        ensure!(c.version == 1, "a new claim must have version 1");
        ensure!(
            c.status == ClaimStatus::Proposed,
            "a new claim must have status PROPOSED"
        );
        ensure!(
            !self.bs(&ctx.branch).claims.contains_key(&c.claim_id),
            "claim {} already exists on branch {}",
            c.claim_id,
            ctx.branch
        );
        self.check_claim_body(ctx, c)?;
        self.check_relations(ctx, &c.claim_id, &p.relations)?;
        let prov = ctx.provenance();
        self.bs_mut(&ctx.branch).claims.insert(
            c.claim_id.clone(),
            ClaimRecord {
                current: c.clone(),
                versions: vec![ClaimVersionRecord {
                    claim: c.clone(),
                    provenance: prov.clone(),
                    rationale: None,
                }],
                created: prov,
                superseded_by: None,
                status_history: vec![StatusChange {
                    version: 1,
                    status: ClaimStatus::Proposed,
                    event: ctx.id,
                    reason: "created".into(),
                }],
            },
        );
        self.record_claim_edges(ctx, c, &p.relations);
        Ok(())
    }

    fn claim_revise(&mut self, ctx: &Ctx, p: &ClaimRevise) -> Result<()> {
        let c = &p.claim;
        let rec = self
            .bs(&ctx.branch)
            .claims
            .get(&c.claim_id)
            .ok_or_else(|| {
                ProjectionError::Rejected(format!("claim {} does not exist", c.claim_id))
            })?;
        ensure!(
            rec.created.author == ctx.author,
            "only the claim's author may revise it; others may branch or object (decisions D-013)"
        );
        ensure!(
            rec.superseded_by.is_none(),
            "a superseded claim cannot be revised"
        );
        ensure!(
            c.version == rec.current.version + 1,
            "revision must have version {}",
            rec.current.version + 1
        );
        ensure!(
            c.status == ClaimStatus::Proposed,
            "a revision starts at status PROPOSED (decisions D-014)"
        );
        self.check_claim_body(ctx, c)?;
        self.check_relations(ctx, &c.claim_id, &p.relations)?;
        let prov = ctx.provenance();
        let rec = self
            .bs_mut(&ctx.branch)
            .claims
            .get_mut(&c.claim_id)
            .unwrap();
        rec.current = c.clone();
        rec.versions.push(ClaimVersionRecord {
            claim: c.clone(),
            provenance: prov,
            rationale: p.rationale.clone(),
        });
        rec.status_history.push(StatusChange {
            version: c.version,
            status: ClaimStatus::Proposed,
            event: ctx.id,
            reason: "revised".into(),
        });
        self.record_claim_edges(ctx, c, &p.relations);
        Ok(())
    }

    fn claim_supersede(&mut self, ctx: &Ctx, p: &ClaimSupersede) -> Result<()> {
        ensure!(
            p.claim_id != p.superseded_by,
            "a claim cannot supersede itself"
        );
        let b = self.bs(&ctx.branch);
        let old = b.claims.get(&p.claim_id).ok_or_else(|| {
            ProjectionError::Rejected(format!("claim {} does not exist", p.claim_id))
        })?;
        ensure!(
            b.claims.contains_key(&p.superseded_by),
            "claim {} does not exist",
            p.superseded_by
        );
        ensure!(
            old.superseded_by.is_none(),
            "claim {} is already superseded",
            p.claim_id
        );
        ensure!(
            old.created.author == ctx.author,
            "only the claim's author may supersede it"
        );
        let rec = self
            .bs_mut(&ctx.branch)
            .claims
            .get_mut(&p.claim_id)
            .unwrap();
        rec.superseded_by = Some(p.superseded_by.clone());
        rec.current.status = ClaimStatus::Superseded;
        rec.status_history.push(StatusChange {
            version: rec.current.version,
            status: ClaimStatus::Superseded,
            event: ctx.id,
            reason: format!("superseded by {}", p.superseded_by),
        });
        self.add_edge(
            ctx,
            ObjectId::claim(&p.superseded_by),
            RelationType::Supersedes,
            ObjectId::claim(&p.claim_id),
        );
        Ok(())
    }

    // ---------------------------------------------------- evidence & objections

    fn evidence_add(&mut self, ctx: &Ctx, e: &Evidence) -> Result<()> {
        let b = self.bs(&ctx.branch);
        ensure!(
            !b.evidence.contains_key(&e.evidence_id),
            "evidence {} already exists",
            e.evidence_id
        );
        ensure!(
            b.claims.contains_key(&e.claim_id),
            "claim {} does not exist",
            e.claim_id
        );
        ensure!(
            !e.description.trim().is_empty(),
            "evidence description must not be empty"
        );
        self.require_artifacts(&e.artifacts)?;
        if let Some(r) = &e.simulation_result {
            ensure!(
                self.simulation_results.contains_key(r),
                "simulation result {r} does not exist"
            );
        }
        if let Some(r) = &e.research_result {
            ensure!(
                self.research_results.contains_key(r),
                "research result {r} does not exist"
            );
        }
        let src = ObjectId::new(ObjectKind::Evidence, &e.evidence_id);
        self.bs_mut(&ctx.branch).evidence.insert(
            e.evidence_id.clone(),
            EvidenceRecord {
                evidence: e.clone(),
                provenance: ctx.provenance(),
                retracted: None,
            },
        );
        self.add_edge(
            ctx,
            src.clone(),
            e.relation.as_relation(),
            ObjectId::claim(&e.claim_id),
        );
        if let Some(r) = &e.simulation_result {
            self.add_edge(
                ctx,
                src.clone(),
                RelationType::DerivesFrom,
                ObjectId::new(ObjectKind::SimulationResult, r),
            );
        }
        if let Some(r) = &e.research_result {
            self.add_edge(
                ctx,
                src,
                RelationType::DerivesFrom,
                ObjectId::new(ObjectKind::ResearchResult, r),
            );
        }
        Ok(())
    }

    fn evidence_retract(&mut self, ctx: &Ctx, p: &EvidenceRetract) -> Result<()> {
        let rec = self
            .bs(&ctx.branch)
            .evidence
            .get(&p.evidence_id)
            .ok_or_else(|| {
                ProjectionError::Rejected(format!("evidence {} does not exist", p.evidence_id))
            })?;
        ensure!(
            rec.provenance.author == ctx.author,
            "only the evidence's author may retract it"
        );
        ensure!(
            rec.retracted.is_none(),
            "evidence {} is already retracted",
            p.evidence_id
        );
        ensure!(!p.reason.trim().is_empty(), "a retraction needs a reason");
        self.bs_mut(&ctx.branch)
            .evidence
            .get_mut(&p.evidence_id)
            .unwrap()
            .retracted = Some(p.reason.clone());
        Ok(())
    }

    fn objection_add(&mut self, ctx: &Ctx, o: &Objection) -> Result<()> {
        ensure!(
            !self
                .bs(&ctx.branch)
                .objections
                .contains_key(&o.objection_id),
            "objection {} already exists",
            o.objection_id
        );
        ensure!(
            self.object_exists(&ctx.branch, &o.target),
            "objection target {} does not exist",
            o.target
        );
        ensure!(
            !o.text.trim().is_empty(),
            "objection text must not be empty"
        );
        self.bs_mut(&ctx.branch).objections.insert(
            o.objection_id.clone(),
            ObjectionRecord {
                objection: o.clone(),
                provenance: ctx.provenance(),
                replies: vec![],
                resolved: false,
            },
        );
        Ok(())
    }

    fn objection_reply(&mut self, ctx: &Ctx, p: &ObjectionReply) -> Result<()> {
        let rec = self
            .bs(&ctx.branch)
            .objections
            .get(&p.objection_id)
            .ok_or_else(|| {
                ProjectionError::Rejected(format!("objection {} does not exist", p.objection_id))
            })?;
        ensure!(!p.text.trim().is_empty(), "reply text must not be empty");
        if p.resolves {
            ensure!(
                rec.provenance.author == ctx.author,
                "only the objection's author can mark it resolved (decisions D-015)"
            );
            ensure!(
                !rec.resolved,
                "objection {} is already resolved",
                p.objection_id
            );
        }
        let prov = ctx.provenance();
        let rec = self
            .bs_mut(&ctx.branch)
            .objections
            .get_mut(&p.objection_id)
            .unwrap();
        rec.replies.push(ReplyRecord {
            text: p.text.clone(),
            resolves: p.resolves,
            provenance: prov,
        });
        if p.resolves {
            rec.resolved = true;
        }
        Ok(())
    }

    // ------------------------------------------------------------ replications

    fn replication_register(&mut self, ctx: &Ctx, r: &Replication) -> Result<()> {
        ensure!(
            !self
                .bs(&ctx.branch)
                .replications
                .contains_key(&r.replication_id),
            "replication {} already exists",
            r.replication_id
        );
        ensure!(
            matches!(
                r.target.kind,
                ObjectKind::Evidence | ObjectKind::SimulationResult | ObjectKind::ResearchResult
            ),
            "a replication must target evidence, a simulation result or a research result"
        );
        ensure!(
            self.object_exists(&ctx.branch, &r.target),
            "replication target {} does not exist",
            r.target
        );
        ensure!(
            !r.protocol.trim().is_empty(),
            "replication protocol must not be empty"
        );
        self.bs_mut(&ctx.branch).replications.insert(
            r.replication_id.clone(),
            ReplicationRecord {
                replication: r.clone(),
                provenance: ctx.provenance(),
                outcome: None,
                result_event: None,
                result_artifacts: vec![],
            },
        );
        Ok(())
    }

    fn replication_result(&mut self, ctx: &Ctx, p: &ReplicationResultPayload) -> Result<()> {
        let rec = self
            .bs(&ctx.branch)
            .replications
            .get(&p.replication_id)
            .ok_or_else(|| {
                ProjectionError::Rejected(format!(
                    "replication {} does not exist",
                    p.replication_id
                ))
            })?;
        ensure!(
            rec.provenance.author == ctx.author,
            "only the replicator may report the result"
        );
        ensure!(
            rec.outcome.is_none(),
            "replication {} already has a result",
            p.replication_id
        );
        self.require_artifacts(&p.artifacts)?;
        let target = rec.replication.target.clone();
        let src = ObjectId::new(ObjectKind::Replication, &p.replication_id);
        let rec = self
            .bs_mut(&ctx.branch)
            .replications
            .get_mut(&p.replication_id)
            .unwrap();
        rec.outcome = Some(p.outcome);
        rec.result_event = Some(ctx.id);
        rec.result_artifacts = p.artifacts.clone();
        if p.outcome == ReplicationOutcome::Success {
            self.add_edge(ctx, src, RelationType::Replicates, target);
        }
        Ok(())
    }

    // -------------------------------------------------------------- validation

    fn validation_submit(&mut self, ctx: &Ctx, v: &ValidationRecord) -> Result<()> {
        ensure!(
            !self
                .bs(&ctx.branch)
                .validations
                .contains_key(&v.validation_id),
            "validation {} already exists",
            v.validation_id
        );
        ensure!(
            self.object_exists(&ctx.branch, &v.target),
            "validation target {} does not exist",
            v.target
        );
        let domains = self.domains_of(&ctx.branch, &v.target);
        authorize_validator(
            &self.validators,
            &v.validator,
            &ctx.author,
            ctx.at,
            &domains,
        )?;
        let duplicate = self.bs(&ctx.branch).validations.values().any(|e| {
            e.record.validator == v.validator
                && e.record.target == v.target
                && e.record.validation_type == v.validation_type
        });
        ensure!(
            !duplicate,
            "validator {} already submitted this validation",
            v.validator
        );

        // Research results earn protocol credit once enough validators pass
        // them, whatever their outcome. Validation never changes a claim's status.
        if v.target.kind == ObjectKind::ResearchResult && v.outcome == ValidationOutcome::Pass {
            let key = EventId(v.target.id.parse()?);
            let min = self.network.canonicalization.minimum_validators;
            let rec = self
                .research_results
                .get_mut(&key)
                .expect("existence checked");
            rec.passing_validators.insert(v.validator.clone());
            if !rec.credited {
                let credit = eel_xchange::credit_for_result(
                    rec.passing_validators.len(),
                    min,
                    rec.result.outcome,
                );
                if credit > 0 {
                    rec.credited = true;
                    let who = rec.result.submitted_by;
                    *self.protocol_credit.entry(who).or_insert(0) += credit;
                }
            }
        }

        self.bs_mut(&ctx.branch).validations.insert(
            v.validation_id.clone(),
            ValidationEntry {
                record: v.clone(),
                provenance: ctx.provenance(),
            },
        );
        Ok(())
    }

    // ---------------------------------------------------------------- branches

    fn branch_create(&mut self, ctx: &Ctx, p: &OntologyBranch) -> Result<()> {
        ensure!(
            !self.branches.contains_key(&p.branch_id),
            "branch {} already exists",
            p.branch_id
        );
        ensure!(
            p.parent_branch.as_ref() == Some(&ctx.branch),
            "BRANCH_CREATE must be recorded on the parent branch it forks from"
        );
        let parent = &self.branches[&ctx.branch];
        ensure!(
            p.fork_event == parent.head,
            "fork_event must be the parent branch's current head (decisions D-016)"
        );
        ensure!(
            p.created_by == ctx.author,
            "created_by must be the event author"
        );
        ensure!(!p.name.trim().is_empty(), "branch name must not be empty");
        // The child starts with a copy of the parent's ontology and shares its
        // history up to the fork; nothing in the parent is changed or removed.
        let record = BranchRecord {
            branch: p.clone(),
            head: None,
            event_count: 0,
            state: parent.state.clone(),
        };
        self.branches.insert(p.branch_id.clone(), record);
        Ok(())
    }

    fn merge_propose(&mut self, ctx: &Ctx, p: &BranchMergePropose) -> Result<()> {
        ensure!(
            !self.merge_proposals.contains_key(&p.proposal_id),
            "merge proposal {} already exists",
            p.proposal_id
        );
        ensure!(p.source != p.target, "cannot merge a branch into itself");
        for b in [&p.source, &p.target] {
            ensure!(self.branches.contains_key(b), "branch {b} does not exist");
        }
        // TODO(EEL-FUTURE): merge execution. The MVP only records proposals.
        self.merge_proposals.insert(
            p.proposal_id.clone(),
            MergeProposalRecord {
                proposal: p.clone(),
                provenance: ctx.provenance(),
            },
        );
        Ok(())
    }

    // -------------------------------------------------------- canonicalization

    fn canon_propose(&mut self, ctx: &Ctx, p: &CanonicalizationProposal) -> Result<()> {
        let b = self.bs(&ctx.branch);
        ensure!(
            !b.proposals.contains_key(&p.proposal_id),
            "proposal {} already exists",
            p.proposal_id
        );
        let claim = b.claims.get(&p.target_claim).ok_or_else(|| {
            ProjectionError::Rejected(format!("claim {} does not exist", p.target_claim))
        })?;
        ensure!(
            claim.superseded_by.is_none(),
            "a superseded claim cannot be canonicalized"
        );
        ensure!(
            claim.current.version == p.target_version,
            "target_version must be the claim's current version"
        );
        ensure!(
            !matches!(
                p.proposed_status,
                ClaimStatus::Proposed | ClaimStatus::Superseded
            ),
            "canonicalization cannot set PROPOSED or SUPERSEDED"
        );
        for e in &p.evidence {
            let ev = b
                .evidence
                .get(e)
                .ok_or_else(|| ProjectionError::Rejected(format!("evidence {e} does not exist")))?;
            ensure!(
                ev.evidence.claim_id == p.target_claim,
                "evidence {e} is not attached to {}",
                p.target_claim
            );
            ensure!(ev.retracted.is_none(), "evidence {e} has been retracted");
        }
        for v in &p.validations {
            ensure!(
                b.validations.contains_key(v),
                "validation {v} does not exist"
            );
        }
        for r in &p.successful_replications {
            let rep = b.replications.get(r).ok_or_else(|| {
                ProjectionError::Rejected(format!("replication {r} does not exist"))
            })?;
            ensure!(
                rep.outcome == Some(ReplicationOutcome::Success),
                "replication {r} has not succeeded"
            );
        }
        for o in &p.unresolved_objections {
            ensure!(b.objections.contains_key(o), "objection {o} does not exist");
        }
        self.bs_mut(&ctx.branch).proposals.insert(
            p.proposal_id.clone(),
            ProposalRecord {
                proposal: p.clone(),
                provenance: ctx.provenance(),
                accepts: BTreeSet::new(),
                rejects: BTreeSet::new(),
                state: ProposalState::Open,
                blockers: vec![],
            },
        );
        Ok(())
    }

    /// Distinct replicators of the proposal's listed successful replications
    /// whose replicated object is one of the proposal's evidence items (or
    /// what that evidence rests on), excluding the claim's author and the
    /// authors of the replicated objects (decisions D-019).
    fn independent_replications(&self, branch: &BranchId, p: &CanonicalizationProposal) -> usize {
        let b = self.bs(branch);
        let Some(claim) = b.claims.get(&p.target_claim) else {
            return 0;
        };
        let mut replicators = BTreeSet::new();
        for rid in &p.successful_replications {
            let Some(rep) = b.replications.get(rid) else {
                continue;
            };
            if rep.outcome != Some(ReplicationOutcome::Success) {
                continue;
            }
            let target = &rep.replication.target;
            let target_author = match target.kind {
                ObjectKind::Evidence => target
                    .id
                    .parse::<EvidenceId>()
                    .ok()
                    .filter(|e| p.evidence.contains(e))
                    .and_then(|e| b.evidence.get(&e))
                    .filter(|e| e.retracted.is_none())
                    .map(|e| e.provenance.author),
                ObjectKind::SimulationResult => target
                    .id
                    .parse::<Hash>()
                    .ok()
                    .map(SimulationResultId)
                    .filter(|s| {
                        p.evidence.iter().any(|e| {
                            b.evidence
                                .get(e)
                                .is_some_and(|e| e.evidence.simulation_result == Some(*s))
                        })
                    })
                    .and_then(|s| self.simulation_results.get(&s))
                    .map(|s| s.provenance.author),
                ObjectKind::ResearchResult => target
                    .id
                    .parse::<Hash>()
                    .ok()
                    .map(EventId)
                    .filter(|r| {
                        p.evidence.iter().any(|e| {
                            b.evidence
                                .get(e)
                                .is_some_and(|e| e.evidence.research_result == Some(*r))
                        })
                    })
                    .and_then(|r| self.research_results.get(&r))
                    .map(|r| r.provenance.author),
                _ => None,
            };
            let Some(target_author) = target_author else {
                continue;
            };
            let who = rep.provenance.author;
            if who != claim.created.author && who != target_author {
                replicators.insert(who);
            }
        }
        replicators.len()
    }

    fn unresolved_critical(&self, branch: &BranchId, p: &CanonicalizationProposal) -> usize {
        let b = self.bs(branch);
        let claim_ref = ObjectId::claim(&p.target_claim);
        let evidence_refs: BTreeSet<ObjectId> = b
            .evidence
            .values()
            .filter(|e| e.evidence.claim_id == p.target_claim)
            .map(|e| ObjectId::new(ObjectKind::Evidence, &e.evidence.evidence_id))
            .collect();
        b.objections
            .values()
            .filter(|o| !o.resolved && o.objection.severity == ObjectionSeverity::Critical)
            .filter(|o| {
                o.objection.target == claim_ref || evidence_refs.contains(&o.objection.target)
            })
            .count()
    }

    fn canon_vote(&mut self, ctx: &Ctx, d: &CanonicalizationDecision, accept: bool) -> Result<()> {
        let rec = self
            .bs(&ctx.branch)
            .proposals
            .get(&d.proposal_id)
            .ok_or_else(|| {
                ProjectionError::Rejected(format!("proposal {} does not exist", d.proposal_id))
            })?;
        ensure!(
            rec.state == ProposalState::Open,
            "proposal {} is no longer open",
            d.proposal_id
        );
        ensure!(
            !rec.accepts.contains(&d.validator) && !rec.rejects.contains(&d.validator),
            "validator {} has already voted on {}",
            d.validator,
            d.proposal_id
        );
        let proposal = rec.proposal.clone();
        let domains = self.claim_domains(&ctx.branch, std::slice::from_ref(&proposal.target_claim));
        authorize_validator(
            &self.validators,
            &d.validator,
            &ctx.author,
            ctx.at,
            &domains,
        )?;

        let rule = self.network.canonicalization.clone();
        let rec = self
            .bs_mut(&ctx.branch)
            .proposals
            .get_mut(&d.proposal_id)
            .unwrap();
        if accept {
            rec.accepts.insert(d.validator.clone());
        } else {
            rec.rejects.insert(d.validator.clone());
        }
        let accepts = rec.accepts.len();
        let rejects = rec.rejects.len();

        if !accept {
            if rejects >= rule.minimum_validators as usize {
                rec.state = ProposalState::Rejected;
                rec.blockers = vec![];
            }
            return Ok(());
        }

        let current_version = self.bs(&ctx.branch).claims[&proposal.target_claim]
            .current
            .version;
        let facts = ProposalFacts {
            accepting_validators: accepts,
            independent_replications: self.independent_replications(&ctx.branch, &proposal),
            unresolved_critical_objections: self.unresolved_critical(&ctx.branch, &proposal),
            version_is_current: current_version == proposal.target_version,
        };
        let blockers = canonicalization_blockers(&rule, &facts);
        let bs = self.bs_mut(&ctx.branch);
        let rec = bs.proposals.get_mut(&d.proposal_id).unwrap();
        rec.blockers = blockers.clone();
        if blockers.is_empty() {
            rec.state = ProposalState::Accepted;
            let claim = bs.claims.get_mut(&proposal.target_claim).unwrap();
            claim.current.status = proposal.proposed_status;
            claim.status_history.push(StatusChange {
                version: claim.current.version,
                status: proposal.proposed_status,
                event: ctx.id,
                reason: format!("canonicalization {}", proposal.proposal_id),
            });
        }
        Ok(())
    }

    // --------------------------------------------------------------- artifacts

    fn artifact_register(&mut self, ctx: &Ctx, a: &Artifact) -> Result<()> {
        ensure!(
            !self.artifacts.contains_key(&a.artifact_id),
            "artifact {} is already registered",
            a.artifact_id
        );
        ensure!(
            a.content_hash == format!("blake3:{}", a.artifact_id),
            "content_hash must be `blake3:<artifact_id>`"
        );
        ensure!(
            a.author == ctx.author,
            "artifact author must be the event author"
        );
        ensure!(!a.uri.trim().is_empty(), "artifact uri must not be empty");
        self.artifacts.insert(a.artifact_id, a.clone());
        Ok(())
    }

    // -------------------------------------------------------------- simulation

    fn simulation_define(&mut self, ctx: &Ctx, d: &SimulationDefine) -> Result<()> {
        let j = &d.job;
        ensure!(
            !self.simulation_jobs.contains_key(&j.job_id),
            "simulation job {} already exists",
            j.job_id
        );
        ensure!(
            j.seed_start <= j.seed_end,
            "seed_start must not exceed seed_end"
        );
        ensure!(
            j.steps > 0 && j.steps <= MAX_SIMULATION_STEPS,
            "steps must be 1..={MAX_SIMULATION_STEPS}"
        );
        ensure!(
            !j.metrics.is_empty(),
            "a job must request at least one metric"
        );
        self.require_claims(&ctx.branch, &j.linked_claims)?;
        // Construction checks engine version and both hashes.
        RelationalGraphEngine::new(
            j.clone(),
            d.ruleset.clone(),
            d.initial_conditions.clone(),
            ctx.author,
        )?;
        self.simulation_jobs.insert(
            j.job_id.clone(),
            SimulationJobRecord {
                definition: d.clone(),
                provenance: ctx.provenance(),
            },
        );
        Ok(())
    }

    fn engine_for(
        &self,
        job: &SimulationJobId,
        worker: IdentityId,
    ) -> Result<RelationalGraphEngine> {
        let rec = self.simulation_jobs.get(job).ok_or_else(|| {
            ProjectionError::Rejected(format!("simulation job {job} does not exist"))
        })?;
        let d = &rec.definition;
        Ok(RelationalGraphEngine::new(
            d.job.clone(),
            d.ruleset.clone(),
            d.initial_conditions.clone(),
            worker,
        )?)
    }

    fn simulation_result(&mut self, ctx: &Ctx, r: &SimulationResult) -> Result<()> {
        ensure!(
            r.worker == ctx.author,
            "the worker must be the event author"
        );
        ensure!(
            r.result_id == r.compute_id()?,
            "result_id does not match the result's content"
        );
        ensure!(
            !self.simulation_results.contains_key(&r.result_id),
            "simulation result {} already exists",
            r.result_id
        );
        let job = self.simulation_jobs.get(&r.job_id).ok_or_else(|| {
            ProjectionError::Rejected(format!("simulation job {} does not exist", r.job_id))
        })?;
        ensure!(
            r.seed >= job.definition.job.seed_start && r.seed <= job.definition.job.seed_end,
            "seed outside the job's range"
        );
        ensure!(
            r.ruleset_hash == job.definition.job.ruleset_hash,
            "ruleset hash does not match the job"
        );
        ensure!(
            r.engine_hash == eel_simulation::engine_hash(),
            "engine hash does not match this engine"
        );
        self.require_artifacts(&[r.result_artifact])?;
        // Results are recorded as submitted. They are checked by replay when
        // validators verify them, not here (decisions D-021).
        self.simulation_results.insert(
            r.result_id,
            SimulationResultRecord {
                result: r.clone(),
                provenance: ctx.provenance(),
                verifications: Default::default(),
            },
        );
        Ok(())
    }

    fn simulation_verify(&mut self, ctx: &Ctx, v: &SimulationVerify) -> Result<()> {
        let rec = self.simulation_results.get(&v.result_id).ok_or_else(|| {
            ProjectionError::Rejected(format!("simulation result {} does not exist", v.result_id))
        })?;
        ensure!(
            !rec.verifications.contains_key(&v.validator),
            "validator {} already verified this result",
            v.validator
        );
        ensure!(
            rec.result.worker != ctx.author,
            "a worker cannot verify their own result"
        );
        let domains = self.domains_of(
            &ctx.branch,
            &ObjectId::new(ObjectKind::SimulationResult, v.result_id),
        );
        authorize_validator(
            &self.validators,
            &v.validator,
            &ctx.author,
            ctx.at,
            &domains,
        )?;
        // Every node replays the result itself; a verification that misreports
        // the replay is rejected, so the ledger cannot hold a false verification.
        let engine = self.engine_for(&rec.result.job_id, rec.result.worker)?;
        let reproduced = engine.verify(&rec.result)?;
        ensure!(
            reproduced == v.reproduced,
            "verification reports reproduced={} but replay gives {}",
            v.reproduced,
            reproduced
        );
        self.simulation_results
            .get_mut(&v.result_id)
            .unwrap()
            .verifications
            .insert(v.validator.clone(), reproduced);
        Ok(())
    }

    // ---------------------------------------------------------------- research

    fn research_request(&mut self, ctx: &Ctx, r: &ResearchRequest) -> Result<()> {
        ensure!(
            !self.research_requests.contains_key(&r.request_id),
            "research request {} already exists",
            r.request_id
        );
        ensure!(
            r.created_by == ctx.author,
            "created_by must be the event author"
        );
        ensure!(
            !r.title.trim().is_empty(),
            "request title must not be empty"
        );
        ensure!(
            !r.requested_work.is_empty(),
            "a request must name at least one work type"
        );
        self.require_claims(&ctx.branch, &r.linked_claims)?;
        self.research_requests.insert(
            r.request_id.clone(),
            ResearchRequestRecord {
                request: r.clone(),
                provenance: ctx.provenance(),
                commitments: vec![],
                results: vec![],
            },
        );
        Ok(())
    }

    fn research_support(&mut self, ctx: &Ctx, c: &SupportCommitment) -> Result<()> {
        ensure!(
            c.supporter == ctx.author,
            "supporter must be the event author"
        );
        ensure!(c.amount_units > 0, "amount_units must be positive");
        let rec = self
            .research_requests
            .get_mut(&c.request_id)
            .ok_or_else(|| {
                ProjectionError::Rejected(format!(
                    "research request {} does not exist",
                    c.request_id
                ))
            })?;
        rec.commitments.push(c.clone());
        Ok(())
    }

    fn research_result(&mut self, ctx: &Ctx, r: &ResearchResult) -> Result<()> {
        ensure!(
            r.submitted_by == ctx.author,
            "submitted_by must be the event author"
        );
        ensure!(
            !r.methodology.trim().is_empty(),
            "methodology must not be empty"
        );
        ensure!(
            self.research_requests.contains_key(&r.request_id),
            "research request {} does not exist",
            r.request_id
        );
        self.require_artifacts(&r.artifacts)?;
        self.require_claims(&ctx.branch, &r.linked_claims)?;
        self.research_requests
            .get_mut(&r.request_id)
            .unwrap()
            .results
            .push(ctx.id);
        self.research_results.insert(
            ctx.id,
            ResearchResultRecord {
                result: r.clone(),
                provenance: ctx.provenance(),
                passing_validators: BTreeSet::new(),
                credited: false,
            },
        );
        Ok(())
    }

    fn xchange_asset(&mut self, ctx: &Ctx, a: &XchangeAsset) -> Result<()> {
        ensure!(
            !self.xchange_assets.contains_key(&a.asset_id),
            "asset {} already exists",
            a.asset_id
        );
        ensure!(
            a.provider == ctx.author,
            "provider must be the event author"
        );
        if let Some(r) = &a.artifact_ref {
            self.require_artifacts(&[*r])?;
        }
        for l in &a.ontology_links {
            ensure!(
                self.object_exists(&ctx.branch, l),
                "linked object {l} does not exist"
            );
        }
        self.xchange_assets
            .insert(a.asset_id.clone(), (a.clone(), ctx.provenance()));
        Ok(())
    }
}

/// Event types whose effects are scoped to the event's branch. All other
/// event types act on network-wide registries.
pub fn is_branch_scoped(t: EventType) -> bool {
    use EventType::*;
    matches!(
        t,
        ClaimCreate
            | ClaimRevise
            | ClaimSupersede
            | EvidenceAdd
            | EvidenceRetract
            | ObjectionAdd
            | ObjectionReply
            | ReplicationRegister
            | ReplicationResult
            | ValidationSubmit
            | BranchCreate
            | CanonicalizationPropose
            | CanonicalizationAccept
            | CanonicalizationReject
    )
}
