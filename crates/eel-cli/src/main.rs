//! `eel`: the command-line client.
//!
//! Commands read YAML inputs, sign events with a local identity, and submit
//! them to the node database in the data directory, which seals one block per
//! accepted event (decisions D-007, D-029).

mod inputs;

use clap::{Args, Parser, Subcommand, ValueEnum};
use eel_api::datadir;
use eel_core::event::*;
use eel_core::research::*;
use eel_core::simulation::SimulationResult;
use eel_core::xchange::Artifact;
use eel_core::*;
use eel_crypto::KeyPair;
use eel_storage::PersistentNode;
use inputs::*;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

type CliResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(
    name = "eel",
    version,
    about = "Emergent Epistemic Ledger command-line client"
)]
struct Cli {
    #[command(flatten)]
    global: Global,
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Clone)]
struct Global {
    /// Data directory (default: $EEL_HOME or ./.eel).
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    /// Identity to act as (default: the keystore's default identity).
    #[arg(long, global = true)]
    identity: Option<String>,
    /// Branch to act on.
    #[arg(long, global = true, default_value = "main")]
    branch: String,
    /// Print JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Manage local identities.
    #[command(subcommand)]
    Identity(IdentityCmd),
    /// Run the node.
    #[command(subcommand)]
    Node(NodeCmd),
    /// Claims.
    #[command(subcommand)]
    Claim(ClaimCmd),
    /// Evidence.
    #[command(subcommand)]
    Evidence(EvidenceCmd),
    /// Objections.
    #[command(subcommand)]
    Objection(ObjectionCmd),
    /// Replications.
    #[command(subcommand)]
    Replication(ReplicationCmd),
    /// Genesis validators.
    #[command(subcommand)]
    Validator(ValidatorCmd),
    /// Submit a validation record (as a validator identity).
    Validate { file: PathBuf },
    /// Branches.
    #[command(subcommand)]
    Branch(BranchCmd),
    /// Canonicalization proposals and votes.
    #[command(subcommand)]
    Canonical(CanonicalCmd),
    /// Off-ledger artifacts.
    #[command(subcommand)]
    Artifact(ArtifactCmd),
    /// Research requests, support commitments and results.
    #[command(subcommand)]
    Research(ResearchCmd),
    /// Deterministic simulations.
    #[command(subcommand)]
    Sim(SimCmd),
    /// DATA XCHANGE asset envelopes.
    #[command(subcommand)]
    Xchange(XchangeCmd),
    /// Inspect the projected ontology.
    #[command(subcommand)]
    Ontology(OntologyCmd),
    /// Show an event.
    Event { id: String },
    /// Export JSON Schemas for every protocol object.
    Schema {
        #[arg(long, default_value = "schemas")]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum IdentityCmd {
    /// Generate a key pair, store it locally and register the identity.
    Create,
    /// Show a registered identity (default: the current one).
    Show { id: Option<String> },
    /// List local identities.
    List,
    /// Import a key file (for example a development validator key) and register it if needed.
    Import { file: PathBuf },
    /// Make a local identity the default.
    Use { id: String },
}

#[derive(Subcommand)]
enum NodeCmd {
    /// Serve the HTTP API.
    Start {
        #[arg(long, default_value = "127.0.0.1:7878")]
        listen: String,
    },
    /// Show the head block and state root.
    Status,
    /// Write the development genesis and keys to a directory (fixtures).
    DevFixtures { out: PathBuf },
}

#[derive(Subcommand)]
enum ClaimCmd {
    Create {
        file: PathBuf,
    },
    Revise {
        file: PathBuf,
    },
    Supersede {
        claim: String,
        by: String,
        #[arg(long)]
        rationale: Option<String>,
    },
    Show {
        id: String,
    },
    History {
        id: String,
    },
    Dependencies {
        id: String,
    },
}

#[derive(Subcommand)]
enum EvidenceCmd {
    Add {
        file: PathBuf,
    },
    Retract {
        id: String,
        #[arg(long)]
        reason: String,
    },
}

#[derive(Subcommand)]
enum ObjectionCmd {
    Add {
        file: PathBuf,
    },
    Reply {
        id: String,
        text: String,
        #[arg(long)]
        resolves: bool,
    },
}

#[derive(Subcommand)]
enum ReplicationCmd {
    Register { file: PathBuf },
    Result { file: PathBuf },
}

#[derive(Subcommand)]
enum ValidatorCmd {
    List,
}

#[derive(Subcommand)]
enum BranchCmd {
    /// Fork the current --branch into a new branch.
    Create {
        name: String,
    },
    List,
    ProposeMerge {
        id: String,
        source: String,
        target: String,
        #[arg(long)]
        rationale: String,
    },
}

#[derive(Subcommand)]
enum CanonicalCmd {
    Propose {
        file: PathBuf,
    },
    /// Vote to accept (as a validator identity).
    Accept {
        proposal: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Vote to reject (as a validator identity).
    Reject {
        proposal: String,
        #[arg(long)]
        reason: Option<String>,
    },
    Show {
        proposal: String,
    },
}

#[derive(Subcommand)]
enum ArtifactCmd {
    /// Hash a file, keep a copy in the data directory and register it.
    Register {
        path: PathBuf,
        #[arg(long)]
        media_type: Option<String>,
        #[arg(long)]
        license: Option<String>,
    },
    /// Recompute an artifact's hash from its stored copy.
    Verify { id: String },
}

#[derive(Subcommand)]
enum ResearchCmd {
    #[command(subcommand)]
    Request(RequestCmd),
    #[command(subcommand)]
    Result(ResultCmd),
}

#[derive(Subcommand)]
enum RequestCmd {
    Create {
        file: PathBuf,
    },
    Support {
        request: String,
        #[arg(long)]
        units: u64,
        #[arg(long, value_enum, default_value = "execution")]
        pool: Pool,
    },
    Show {
        id: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Pool {
    Execution,
    Replication,
    SupportRisk,
}

#[derive(Subcommand)]
enum ResultCmd {
    Submit { file: PathBuf },
}

#[derive(Subcommand)]
enum SimCmd {
    /// Define the job if needed, run its seeds, and submit every result.
    Run {
        file: PathBuf,
        /// Run only this seed.
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Replay a result locally and record the outcome (as a validator identity).
    Verify { result_id: String },
    /// Replay a result locally without recording anything.
    Check { result_id: String },
}

#[derive(Subcommand)]
enum XchangeCmd {
    Register { file: PathBuf },
    List,
}

#[derive(Subcommand)]
enum OntologyCmd {
    /// Claims and edges on the branch.
    Graph,
    /// State root and counts, now or at a block height.
    State {
        #[arg(long)]
        height: Option<u64>,
    },
    /// Claims whose current version has been canonicalized.
    Canonical,
}

struct Ctx {
    g: Global,
    dir: PathBuf,
}

impl Ctx {
    async fn node(&self) -> CliResult<PersistentNode> {
        datadir::open_node(&self.dir).await
    }

    fn branch(&self) -> CliResult<BranchId> {
        Ok(self.g.branch.parse()?)
    }

    fn key(&self) -> CliResult<KeyPair> {
        let ks = datadir::keystore(&self.dir)?;
        let id = match &self.g.identity {
            Some(s) => resolve_identity(&ks, s)?,
            None => ks.default_identity()?,
        };
        Ok(ks.load(&id)?)
    }

    /// Signs a payload as the current identity on the current branch and submits it.
    async fn submit(&self, node: &mut PersistentNode, payload: EventPayload) -> CliResult<EventId> {
        let key = self.key()?;
        self.submit_as(node, &key, payload).await
    }

    async fn submit_as(
        &self,
        node: &mut PersistentNode,
        key: &KeyPair,
        payload: EventPayload,
    ) -> CliResult<EventId> {
        let branch = self.branch()?;
        let head = node
            .ledger
            .state()
            .branch(&branch)
            .ok_or_else(|| format!("branch {branch} does not exist"))?
            .head;
        // Authors pick their own timestamp; it may not precede the parent's.
        let parent_ts = head
            .and_then(|h| node.ledger.event(&h))
            .map(|e| e.timestamp)
            .unwrap_or_default();
        let ts = Timestamp(Timestamp::now().0.max(parent_ts.0));
        let (_, event) = key.author_event(ts, branch, head.into_iter().collect(), payload)?;
        let id = node.submit(event).await?;
        let h = node.ledger.head();
        if self.g.json {
            println!(
                "{}",
                serde_json::json!({"event_id": id, "block_height": h.block.height, "state_root": h.block.state_root})
            );
        } else {
            println!("accepted event {id}");
            println!(
                "  block {} · state root {}",
                h.block.height, h.block.state_root
            );
        }
        Ok(id)
    }

    fn print<T: serde::Serialize>(&self, v: &T) -> CliResult {
        if self.g.json {
            println!("{}", serde_json::to_string_pretty(v)?);
        } else {
            print!("{}", serde_yaml::to_string(v)?);
        }
        Ok(())
    }
}

fn resolve_identity(ks: &eel_crypto::Keystore, s: &str) -> CliResult<IdentityId> {
    if let Ok(id) = s.parse::<IdentityId>() {
        return Ok(id);
    }
    // Allow a unique prefix, or a development validator name.
    if let Some((_, _)) = datadir::DEV_VALIDATORS.iter().find(|(v, _)| *v == s) {
        return Ok(datadir::dev_key(s).identity_id());
    }
    let matches: Vec<_> = ks
        .list()?
        .into_iter()
        .filter(|id| id.to_string().starts_with(s))
        .collect();
    match matches.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("no local identity matches `{s}`").into()),
        _ => Err(format!("`{s}` matches several identities").into()),
    }
}

fn read_yaml<T: DeserializeOwned>(path: &Path) -> CliResult<T> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?)
}

/// The validator id whose identity is `key`.
fn validator_for(node: &PersistentNode, key: &KeyPair) -> CliResult<ValidatorId> {
    node.ledger
        .state()
        .validators
        .values()
        .find(|v| v.identity_id == key.identity_id())
        .map(|v| v.validator_id.clone())
        .ok_or_else(|| "the current identity is not a validator; use --identity VAL-A (after `eel identity import`)".into())
}

fn artifact_for(
    key: &KeyPair,
    bytes: &[u8],
    uri: String,
    media_type: String,
    license: Option<String>,
) -> Artifact {
    let id = ArtifactId(Hash::digest(bytes));
    Artifact {
        artifact_id: id,
        content_hash: format!("blake3:{id}"),
        uri,
        media_type,
        size_bytes: bytes.len() as u64,
        author: key.identity_id(),
        license,
        external_hashes: vec![format!("sha256:{}", eel_crypto::sha256_hex(bytes))],
    }
}

fn store_artifact(dir: &Path, bytes: &[u8]) -> CliResult<(ArtifactId, PathBuf)> {
    let id = ArtifactId(Hash::digest(bytes));
    let path = datadir::artifacts_dir(dir).join(id.to_string());
    std::fs::write(&path, bytes)?;
    Ok((id, path))
}

fn file_uri(p: &Path) -> String {
    let abs = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    format!("file://{}", abs.display())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> CliResult {
    let cli = Cli::parse();
    let dir = cli
        .global
        .data_dir
        .clone()
        .or_else(|| std::env::var_os("EEL_HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(".eel"));
    let ctx = Ctx { g: cli.global, dir };

    match cli.command {
        Command::Identity(cmd) => identity(&ctx, cmd).await,
        Command::Node(cmd) => node_cmd(&ctx, cmd).await,
        Command::Claim(cmd) => claim(&ctx, cmd).await,
        Command::Evidence(cmd) => {
            let mut node = ctx.node().await?;
            let payload = match cmd {
                EvidenceCmd::Add { file } => EventPayload::EvidenceAdd(read_yaml(&file)?),
                EvidenceCmd::Retract { id, reason } => {
                    EventPayload::EvidenceRetract(EvidenceRetract {
                        evidence_id: id.parse()?,
                        reason,
                    })
                }
            };
            ctx.submit(&mut node, payload).await.map(|_| ())
        }
        Command::Objection(cmd) => {
            let mut node = ctx.node().await?;
            let payload = match cmd {
                ObjectionCmd::Add { file } => EventPayload::ObjectionAdd(read_yaml(&file)?),
                ObjectionCmd::Reply { id, text, resolves } => {
                    EventPayload::ObjectionReply(ObjectionReply {
                        objection_id: id.parse()?,
                        text,
                        resolves,
                    })
                }
            };
            ctx.submit(&mut node, payload).await.map(|_| ())
        }
        Command::Replication(cmd) => {
            let mut node = ctx.node().await?;
            let payload = match cmd {
                ReplicationCmd::Register { file } => {
                    EventPayload::ReplicationRegister(read_yaml(&file)?)
                }
                ReplicationCmd::Result { file } => {
                    EventPayload::ReplicationResult(read_yaml(&file)?)
                }
            };
            ctx.submit(&mut node, payload).await.map(|_| ())
        }
        Command::Validator(ValidatorCmd::List) => {
            let node = ctx.node().await?;
            let validators: Vec<_> = node.ledger.state().validators.values().collect();
            if ctx.g.json {
                return ctx.print(&validators);
            }
            for v in validators {
                println!(
                    "{}  {:?}  domains: {}  identity: {}",
                    v.validator_id,
                    v.validator_class,
                    v.domains.join(", "),
                    v.identity_id
                );
            }
            Ok(())
        }
        Command::Validate { file } => {
            let mut node = ctx.node().await?;
            let mut record: eel_core::validation::ValidationRecord = read_yaml(&file)?;
            let key = ctx.key()?;
            let me = validator_for(&node, &key)?;
            if record.validator != me {
                eprintln!("note: recording as {me}, the validator for the current identity");
                record.validator = me;
            }
            ctx.submit_as(&mut node, &key, EventPayload::ValidationSubmit(record))
                .await
                .map(|_| ())
        }
        Command::Branch(cmd) => branch(&ctx, cmd).await,
        Command::Canonical(cmd) => canonical(&ctx, cmd).await,
        Command::Artifact(cmd) => artifact(&ctx, cmd).await,
        Command::Research(cmd) => research(&ctx, cmd).await,
        Command::Sim(cmd) => sim(&ctx, cmd).await,
        Command::Xchange(cmd) => {
            let mut node = ctx.node().await?;
            match cmd {
                XchangeCmd::Register { file } => {
                    let key = ctx.key()?;
                    let input: XchangeAssetInput = read_yaml(&file)?;
                    ctx.submit_as(
                        &mut node,
                        &key,
                        EventPayload::XchangeAssetRegister(input.into_asset(key.identity_id())),
                    )
                    .await
                    .map(|_| ())
                }
                XchangeCmd::List => {
                    let assets: Vec<_> = node
                        .ledger
                        .state()
                        .xchange_assets
                        .values()
                        .map(|(a, _)| a)
                        .collect();
                    ctx.print(&assets)
                }
            }
        }
        Command::Ontology(cmd) => ontology(&ctx, cmd).await,
        Command::Event { id } => {
            let node = ctx.node().await?;
            let id: EventId = id.parse()?;
            let e = node.ledger.event(&id).ok_or("event not found")?;
            ctx.print(e)
        }
        Command::Schema { out } => export_schemas(&out),
    }
}

async fn identity(ctx: &Ctx, cmd: IdentityCmd) -> CliResult {
    datadir::ensure_initialized(&ctx.dir)?;
    let ks = datadir::keystore(&ctx.dir)?;
    match cmd {
        IdentityCmd::Create => {
            let key = KeyPair::generate()?;
            let id = ks.save(&key)?;
            let mut node = ctx.node().await?;
            let (_, event) = key.identity_register_event(Timestamp::now())?;
            node.submit(event).await?;
            let first_human = ks.default_identity().is_err();
            if first_human {
                ks.set_default(&id)?;
            }
            if ctx.g.json {
                println!(
                    "{}",
                    serde_json::json!({"identity_id": id, "public_key": key.public_key_hex()})
                );
            } else {
                println!("created and registered identity {id}");
                println!("  public key {}", key.public_key_hex());
                println!("  private key stored in {}", ctx.dir.join("keys").display());
                if first_human {
                    println!("  set as the default identity");
                } else {
                    println!(
                        "  use it with --identity {} or `eel identity use {}`",
                        &id.to_string()[..12],
                        &id.to_string()[..12]
                    );
                }
            }
            Ok(())
        }
        IdentityCmd::Show { id } => {
            let node = ctx.node().await?;
            let id = match id {
                Some(s) => resolve_identity(&ks, &s)?,
                None => ctx.key()?.identity_id(),
            };
            let st = node.ledger.state();
            let rec = st.identities.get(&id).ok_or("identity is not registered")?;
            let validator = st.validators.values().find(|v| v.identity_id == id);
            ctx.print(&serde_json::json!({
                "identity": rec,
                "validator": validator,
                "protocol_credit": st.protocol_credit.get(&id).copied().unwrap_or(0),
            }))
        }
        IdentityCmd::List => {
            let default = ks.default_identity().ok();
            for id in ks.list()? {
                let name = datadir::DEV_VALIDATORS
                    .iter()
                    .find(|(v, _)| datadir::dev_key(v).identity_id() == id)
                    .map(|(v, _)| format!(" ({v}, development key)"))
                    .unwrap_or_default();
                let mark = if Some(id) == default { "*" } else { " " };
                println!("{mark} {id}{name}");
            }
            Ok(())
        }
        IdentityCmd::Import { file } => {
            let key = eel_crypto::Keystore::load_file(&file)?;
            let id = ks.save(&key)?;
            let mut node = ctx.node().await?;
            if !node.ledger.state().identities.contains_key(&id) {
                let (_, event) = key.identity_register_event(Timestamp::now())?;
                node.submit(event).await?;
            }
            println!("imported identity {id}");
            Ok(())
        }
        IdentityCmd::Use { id } => {
            let id = resolve_identity(&ks, &id)?;
            ks.load(&id)?;
            ks.set_default(&id)?;
            println!("default identity is now {id}");
            Ok(())
        }
    }
}

async fn node_cmd(ctx: &Ctx, cmd: NodeCmd) -> CliResult {
    match cmd {
        NodeCmd::Start { listen } => eel_api::serve(&ctx.dir, &listen).await,
        NodeCmd::Status => {
            let node = ctx.node().await?;
            let h = node.ledger.head();
            let st = node.ledger.state();
            ctx.print(&serde_json::json!({
                "network": st.network.name,
                "height": h.block.height,
                "block_hash": h.block_hash,
                "state_root": h.block.state_root,
                "events": st.events_applied,
                "identities": st.identities.len(),
                "branches": st.branches.keys().collect::<Vec<_>>(),
            }))
        }
        NodeCmd::DevFixtures { out } => {
            std::fs::create_dir_all(out.join("keys"))?;
            std::fs::write(
                out.join("genesis.yaml"),
                serde_yaml::to_string(&datadir::dev_genesis())?,
            )?;
            let ks = eel_crypto::Keystore::open(out.join("keys"))?;
            for (v, _) in datadir::DEV_VALIDATORS {
                let id = ks.save(&datadir::dev_key(v))?;
                std::fs::rename(
                    out.join("keys").join(format!("{id}.key")),
                    out.join("keys").join(format!("{}.key", v.to_lowercase())),
                )?;
            }
            println!("wrote {}", out.display());
            Ok(())
        }
    }
}

async fn claim(ctx: &Ctx, cmd: ClaimCmd) -> CliResult {
    let mut node = ctx.node().await?;
    let branch = ctx.branch()?;
    let get = |node: &PersistentNode, id: &str| -> CliResult<eel_ontology::ClaimRecord> {
        let id: ClaimId = id.parse()?;
        Ok(node
            .ledger
            .state()
            .branch(&branch)
            .and_then(|b| b.state.claims.get(&id))
            .cloned()
            .ok_or_else(|| format!("claim {id} not found on branch {branch}"))?)
    };
    match cmd {
        ClaimCmd::Create { file } => {
            let input: ClaimInput = read_yaml(&file)?;
            let (claim, relations) = input.into_claim(1);
            ctx.submit(
                &mut node,
                EventPayload::ClaimCreate(ClaimCreate { claim, relations }),
            )
            .await
            .map(|_| ())
        }
        ClaimCmd::Revise { file } => {
            let input: ClaimInput = read_yaml(&file)?;
            let current = get(&node, input.claim_id.as_str())?;
            let rationale = input.rationale.clone();
            let (claim, relations) = input.into_claim(current.current.version + 1);
            ctx.submit(
                &mut node,
                EventPayload::ClaimRevise(ClaimRevise {
                    claim,
                    relations,
                    rationale,
                }),
            )
            .await
            .map(|_| ())
        }
        ClaimCmd::Supersede {
            claim,
            by,
            rationale,
        } => ctx
            .submit(
                &mut node,
                EventPayload::ClaimSupersede(ClaimSupersede {
                    claim_id: claim.parse()?,
                    superseded_by: by.parse()?,
                    rationale,
                }),
            )
            .await
            .map(|_| ()),
        ClaimCmd::Show { id } => {
            let rec = get(&node, &id)?;
            if ctx.g.json {
                return ctx.print(&rec.current);
            }
            let c = &rec.current;
            println!("{} v{} [{:?}] on {branch}", c.claim_id, c.version, c.status);
            println!("  {}", c.title);
            println!("  {}", c.statement);
            println!(
                "  types: {:?} · level: {} · domain: {}",
                c.claim_type, c.ontology_level, c.domain
            );
            if !c.dependencies.is_empty() {
                println!(
                    "  depends on: {}",
                    c.dependencies
                        .iter()
                        .map(|d| d.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            if let Some(s) = &rec.superseded_by {
                println!("  superseded by {s}");
            }
            let bs = &node.ledger.state().branches[&branch].state;
            let ev: Vec<_> = bs
                .evidence
                .values()
                .filter(|e| e.evidence.claim_id == c.claim_id)
                .collect();
            let obj: Vec<_> = bs
                .objections
                .values()
                .filter(|o| o.objection.target == ObjectId::claim(&c.claim_id))
                .collect();
            println!(
                "  evidence: {} · objections: {} ({} unresolved)",
                ev.len(),
                obj.len(),
                obj.iter().filter(|o| !o.resolved).count()
            );
            Ok(())
        }
        ClaimCmd::History { id } => {
            let rec = get(&node, &id)?;
            if ctx.g.json {
                return ctx.print(&rec);
            }
            for v in &rec.versions {
                println!(
                    "v{}  event {}  by {}",
                    v.claim.version, v.provenance.event, v.provenance.author
                );
                println!("    {}", v.claim.statement);
                if let Some(r) = &v.rationale {
                    println!("    rationale: {r}");
                }
            }
            println!("status history:");
            for s in &rec.status_history {
                println!(
                    "  v{} {:?}  ({})  event {}",
                    s.version, s.status, s.reason, s.event
                );
            }
            Ok(())
        }
        ClaimCmd::Dependencies { id } => {
            let id: ClaimId = id.parse()?;
            let deps = node.ledger.state().dependency_closure(&branch, &id);
            ctx.print(&deps)
        }
    }
}

async fn branch(ctx: &Ctx, cmd: BranchCmd) -> CliResult {
    let mut node = ctx.node().await?;
    match cmd {
        BranchCmd::Create { name } => {
            let key = ctx.key()?;
            let parent = ctx.branch()?;
            let head = node
                .ledger
                .state()
                .branch(&parent)
                .ok_or("parent branch not found")?
                .head;
            let payload = EventPayload::BranchCreate(eel_core::ontology::OntologyBranch {
                branch_id: name.parse()?,
                name: name.clone(),
                parent_branch: Some(parent),
                fork_event: head,
                created_by: key.identity_id(),
            });
            ctx.submit_as(&mut node, &key, payload).await.map(|_| ())
        }
        BranchCmd::List => {
            for (id, b) in &node.ledger.state().branches {
                let parent = b
                    .branch
                    .parent_branch
                    .as_ref()
                    .map(|p| format!(" ← {p}"))
                    .unwrap_or_default();
                println!(
                    "{id}{parent}  claims: {}  events: {}",
                    b.state.claims.len(),
                    b.event_count
                );
            }
            Ok(())
        }
        BranchCmd::ProposeMerge {
            id,
            source,
            target,
            rationale,
        } => ctx
            .submit(
                &mut node,
                EventPayload::BranchMergePropose(BranchMergePropose {
                    proposal_id: id.parse()?,
                    source: source.parse()?,
                    target: target.parse()?,
                    rationale,
                }),
            )
            .await
            .map(|_| ()),
    }
}

async fn canonical(ctx: &Ctx, cmd: CanonicalCmd) -> CliResult {
    let mut node = ctx.node().await?;
    match cmd {
        CanonicalCmd::Propose { file } => ctx
            .submit(
                &mut node,
                EventPayload::CanonicalizationPropose(read_yaml(&file)?),
            )
            .await
            .map(|_| ()),
        CanonicalCmd::Accept { proposal, reason } => {
            let key = ctx.key()?;
            let validator = validator_for(&node, &key)?;
            let d = CanonicalizationDecision {
                proposal_id: proposal.parse()?,
                validator,
                reason,
            };
            ctx.submit_as(
                &mut node,
                &key,
                EventPayload::CanonicalizationAccept(d.clone()),
            )
            .await?;
            show_proposal(ctx, &node, &d.proposal_id)
        }
        CanonicalCmd::Reject { proposal, reason } => {
            let key = ctx.key()?;
            let validator = validator_for(&node, &key)?;
            let d = CanonicalizationDecision {
                proposal_id: proposal.parse()?,
                validator,
                reason,
            };
            ctx.submit_as(
                &mut node,
                &key,
                EventPayload::CanonicalizationReject(d.clone()),
            )
            .await?;
            show_proposal(ctx, &node, &d.proposal_id)
        }
        CanonicalCmd::Show { proposal } => show_proposal(ctx, &node, &proposal.parse()?),
    }
}

fn show_proposal(ctx: &Ctx, node: &PersistentNode, id: &CanonicalizationProposalId) -> CliResult {
    let b = ctx.branch()?;
    let p = node
        .ledger
        .state()
        .branch(&b)
        .and_then(|x| x.state.proposals.get(id))
        .ok_or("proposal not found")?;
    if ctx.g.json {
        return ctx.print(p);
    }
    println!(
        "proposal {id}: {:?} → {} v{} as {:?}",
        p.state, p.proposal.target_claim, p.proposal.target_version, p.proposal.proposed_status
    );
    println!(
        "  accepted by: {}",
        p.accepts
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    if !p.rejects.is_empty() {
        println!(
            "  rejected by: {}",
            p.rejects
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    for blocker in &p.blockers {
        println!("  waiting on: {blocker}");
    }
    Ok(())
}

async fn artifact(ctx: &Ctx, cmd: ArtifactCmd) -> CliResult {
    let mut node = ctx.node().await?;
    match cmd {
        ArtifactCmd::Register {
            path,
            media_type,
            license,
        } => {
            let key = ctx.key()?;
            let bytes = std::fs::read(&path)?;
            let (_, _) = store_artifact(&ctx.dir, &bytes)?;
            let media = media_type.unwrap_or_else(|| guess_media_type(&path));
            let a = artifact_for(&key, &bytes, file_uri(&path), media, license);
            let id = a.artifact_id;
            if node.ledger.state().artifacts.contains_key(&id) {
                println!("artifact {id} is already registered");
                return Ok(());
            }
            ctx.submit_as(&mut node, &key, EventPayload::ArtifactRegister(a))
                .await?;
            println!("artifact id {id}");
            Ok(())
        }
        ArtifactCmd::Verify { id } => {
            let id: ArtifactId = id.parse()?;
            let a = node
                .ledger
                .state()
                .artifacts
                .get(&id)
                .ok_or("artifact is not registered")?;
            let local = datadir::artifacts_dir(&ctx.dir).join(id.to_string());
            let original = a.uri.strip_prefix("file://").map(PathBuf::from);
            for (label, p) in [("stored copy", Some(local)), ("original uri", original)] {
                let Some(p) = p else { continue };
                match std::fs::read(&p) {
                    Ok(bytes) if ArtifactId(Hash::digest(&bytes)) == id => {
                        println!("{label}: OK ({})", p.display())
                    }
                    Ok(_) => println!(
                        "{label}: MODIFIED, hash no longer matches ({})",
                        p.display()
                    ),
                    Err(e) => println!("{label}: unavailable ({}: {e})", p.display()),
                }
            }
            Ok(())
        }
    }
}

fn guess_media_type(p: &Path) -> String {
    match p.extension().and_then(|e| e.to_str()) {
        Some("json") => "application/json",
        Some("yaml" | "yml") => "application/yaml",
        Some("csv") => "text/csv",
        Some("txt" | "md") => "text/plain",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
    .into()
}

async fn research(ctx: &Ctx, cmd: ResearchCmd) -> CliResult {
    let mut node = ctx.node().await?;
    let key = ctx.key()?;
    match cmd {
        ResearchCmd::Request(RequestCmd::Create { file }) => {
            let input: ResearchRequestInput = read_yaml(&file)?;
            ctx.submit_as(
                &mut node,
                &key,
                EventPayload::ResearchRequestCreate(input.into_request(key.identity_id())),
            )
            .await
            .map(|_| ())
        }
        ResearchCmd::Request(RequestCmd::Support {
            request,
            units,
            pool,
        }) => {
            let commitment_type = match pool {
                Pool::Execution => CommitmentType::Execution,
                Pool::Replication => CommitmentType::Replication,
                Pool::SupportRisk => CommitmentType::SupportRisk,
            };
            let c = SupportCommitment {
                supporter: key.identity_id(),
                request_id: request.parse()?,
                amount_units: units,
                commitment_type,
            };
            ctx.submit_as(&mut node, &key, EventPayload::ResearchRequestSupport(c))
                .await
                .map(|_| ())
        }
        ResearchCmd::Request(RequestCmd::Show { id }) => {
            let id: ResearchRequestId = id.parse()?;
            let st = node.ledger.state();
            let r = st.research_requests.get(&id).ok_or("request not found")?;
            let results: Vec<_> = r
                .results
                .iter()
                .filter_map(|e| st.research_results.get(e))
                .collect();
            ctx.print(&serde_json::json!({
                "request": r.request,
                "pools_development_units": eel_xchange::pool_totals(&r.commitments),
                "results": results,
            }))
        }
        ResearchCmd::Result(ResultCmd::Submit { file }) => {
            let input: ResearchResultInput = read_yaml(&file)?;
            let id = ctx
                .submit_as(
                    &mut node,
                    &key,
                    EventPayload::ResearchResultSubmit(input.into_result(key.identity_id())),
                )
                .await?;
            println!("research result id {id} (validators target it as kind research_result)");
            Ok(())
        }
    }
}

async fn sim(ctx: &Ctx, cmd: SimCmd) -> CliResult {
    let mut node = ctx.node().await?;
    match cmd {
        SimCmd::Run { file, seed } => {
            let key = ctx.key()?;
            let input: SimulationInput = read_yaml(&file)?;
            let def = input.into_define()?;
            let job_id = def.job.job_id.clone();
            match node.ledger.state().simulation_jobs.get(&job_id) {
                Some(existing) if existing.definition != def => {
                    return Err(
                        format!("job {job_id} already exists with a different definition").into(),
                    );
                }
                Some(_) => println!("job {job_id} is already defined"),
                None => {
                    ctx.submit_as(&mut node, &key, EventPayload::SimulationDefine(def.clone()))
                        .await?;
                }
            }
            let engine = eel_simulation::RelationalGraphEngine::new(
                def.job.clone(),
                def.ruleset,
                def.initial_conditions,
                key.identity_id(),
            )?;
            let seeds: Vec<u64> = match seed {
                Some(s) => vec![s],
                None => (def.job.seed_start..=def.job.seed_end).collect(),
            };
            for s in seeds {
                let (result, bytes) = engine.execute_seed(s)?;
                let (aid, path) = store_artifact(&ctx.dir, &bytes)?;
                if !node.ledger.state().artifacts.contains_key(&aid) {
                    let a = artifact_for(
                        &key,
                        &bytes,
                        file_uri(&path),
                        "application/json".into(),
                        None,
                    );
                    ctx.submit_as(&mut node, &key, EventPayload::ArtifactRegister(a))
                        .await?;
                }
                if node
                    .ledger
                    .state()
                    .simulation_results
                    .contains_key(&result.result_id)
                {
                    println!("seed {s}: result {} already recorded", result.result_id);
                    continue;
                }
                ctx.submit_as(
                    &mut node,
                    &key,
                    EventPayload::SimulationResult(result.clone()),
                )
                .await?;
                print_result(&result);
            }
            Ok(())
        }
        SimCmd::Verify { result_id } => {
            let key = ctx.key()?;
            let validator = validator_for(&node, &key)?;
            let (result, reproduced) = replay(&node, &result_id)?;
            println!(
                "replayed result {}: {}",
                result.result_id,
                if reproduced {
                    "REPRODUCED"
                } else {
                    "NOT REPRODUCED"
                }
            );
            let payload = EventPayload::SimulationVerify(SimulationVerify {
                result_id: result.result_id,
                validator,
                reproduced,
            });
            ctx.submit_as(&mut node, &key, payload).await.map(|_| ())
        }
        SimCmd::Check { result_id } => {
            let (result, reproduced) = replay(&node, &result_id)?;
            println!(
                "replayed result {}: {}",
                result.result_id,
                if reproduced {
                    "REPRODUCED"
                } else {
                    "NOT REPRODUCED"
                }
            );
            Ok(())
        }
    }
}

fn replay(node: &PersistentNode, id: &str) -> CliResult<(SimulationResult, bool)> {
    use eel_simulation::UsefulWorkEngine;
    let st = node.ledger.state();
    let id: SimulationResultId = match id.parse() {
        Ok(h) => h,
        Err(_) => {
            let matches: Vec<_> = st
                .simulation_results
                .keys()
                .filter(|k| k.to_string().starts_with(id))
                .collect();
            match matches.as_slice() {
                [one] => **one,
                _ => return Err(format!("no unique simulation result matches `{id}`").into()),
            }
        }
    };
    let rec = st
        .simulation_results
        .get(&id)
        .ok_or("simulation result not found")?;
    let job = &st.simulation_jobs[&rec.result.job_id].definition;
    let engine = eel_simulation::RelationalGraphEngine::new(
        job.job.clone(),
        job.ruleset.clone(),
        job.initial_conditions.clone(),
        rec.result.worker,
    )?;
    let ok = engine.verify(&rec.result)?;
    Ok((rec.result.clone(), ok))
}

fn print_result(r: &SimulationResult) {
    println!("seed {}: result {}", r.seed, r.result_id);
    println!("  final state hash {}", r.final_state_hash);
    for m in &r.metrics {
        println!("  {:<24} {}", format!("{:?}", m.metric), m.value);
    }
}

async fn ontology(ctx: &Ctx, cmd: OntologyCmd) -> CliResult {
    let node = ctx.node().await?;
    let b = ctx.branch()?;
    match cmd {
        OntologyCmd::Graph => {
            let br = node.ledger.state().branch(&b).ok_or("branch not found")?;
            if ctx.g.json {
                return ctx.print(&serde_json::json!({
                    "branch": b,
                    "claims": br.state.claims.values().map(|c| &c.current).collect::<Vec<_>>(),
                    "edges": br.state.edges,
                }));
            }
            println!(
                "branch {b}: {} claims, {} edges",
                br.state.claims.len(),
                br.state.edges.len()
            );
            for c in br.state.claims.values() {
                println!(
                    "  {} v{} [{:?}] {}",
                    c.current.claim_id, c.current.version, c.current.status, c.current.title
                );
            }
            for e in &br.state.edges {
                println!(
                    "  {} --{:?}--> {}   (event {})",
                    e.source,
                    e.relation,
                    e.target,
                    &e.originating_event.to_string()[..12]
                );
            }
            Ok(())
        }
        OntologyCmd::State { height } => {
            let (h, s) = match height {
                Some(h) => (h, node.ledger.state_at(h)?),
                None => (node.ledger.head().block.height, node.ledger.state().clone()),
            };
            ctx.print(&serde_json::json!({
                "height": h,
                "state_root": s.state_root(),
                "identities": s.identities.len(),
                "branches": s.branches.iter().map(|(k, v)| (k.to_string(), serde_json::json!({
                    "claims": v.state.claims.len(),
                    "edges": v.state.edges.len(),
                    "evidence": v.state.evidence.len(),
                    "objections": v.state.objections.len(),
                }))).collect::<std::collections::BTreeMap<_, _>>(),
                "simulation_results": s.simulation_results.len(),
                "research_requests": s.research_requests.len(),
                "protocol_credit": s.protocol_credit,
            }))
        }
        OntologyCmd::Canonical => ctx.print(&node.ledger.state().canonical_claims(&b)),
    }
}

fn export_schemas(out: &Path) -> CliResult {
    use schemars::schema_for;
    macro_rules! write_schema {
        ($dir:literal, $t:ty) => {{
            let dir = out.join($dir);
            std::fs::create_dir_all(&dir)?;
            let name = stringify!($t).rsplit("::").next().unwrap().to_string();
            let schema = schema_for!($t);
            std::fs::write(
                dir.join(format!("{name}.json")),
                serde_json::to_string_pretty(&schema)? + "\n",
            )?;
            println!("wrote {}/{name}.json", dir.display());
        }};
    }
    write_schema!("event", EpistemicEvent);
    write_schema!("event", EventPayload);
    write_schema!("event", EventType);
    write_schema!("ontology", eel_core::ontology::Claim);
    write_schema!("ontology", eel_core::ontology::Evidence);
    write_schema!("ontology", eel_core::ontology::Objection);
    write_schema!("ontology", eel_core::ontology::Replication);
    write_schema!("ontology", eel_core::ontology::OntologyEdge);
    write_schema!("ontology", eel_core::ontology::OntologyBranch);
    write_schema!("ontology", eel_core::ontology::OntologicalDefensibility);
    write_schema!("validation", eel_core::validation::Validator);
    write_schema!("validation", eel_core::validation::ValidationRecord);
    write_schema!("validation", eel_core::validation::CanonicalizationProposal);
    write_schema!("validation", eel_core::genesis::GenesisConfig);
    write_schema!("simulation", eel_core::simulation::SimulationJob);
    write_schema!("simulation", eel_core::simulation::SimulationResult);
    write_schema!("simulation", eel_core::simulation::Ruleset);
    write_schema!("simulation", eel_core::simulation::InitialConditions);
    write_schema!("xchange", eel_core::xchange::XchangeAsset);
    write_schema!("xchange", eel_core::xchange::Artifact);
    write_schema!("xchange", ResearchRequest);
    write_schema!("xchange", SupportCommitment);
    write_schema!("xchange", ResearchResult);
    Ok(())
}
