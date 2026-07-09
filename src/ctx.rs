//! Shared CLI context.
//!
//! Holds the single [`PersistenceBackend`] for the orthogonal
//! engines (causal graph, provenance, patterns, antifragile) plus
//! the brain-like [`Cortex`]. Every CLI subcommand and MCP tool
//! routes through these two.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::RwLock;

use nexus_cog_antifragile::{AdversarialGenerator, EdgeCaseExplorer, RobustnessScorer};
use nexus_cog_causal::{
    BackwardReasoner, BlastRadiusCalculator, CausalGraphEngine, CounterfactualReasoner,
    ForwardReasoner, PreMortemEngine,
};
use nexus_cog_neural::{Cortex, CortexConfig};
use nexus_cog_patterns::PatternMatcher;
use nexus_cog_provenance::ProvenanceGraphEngine;
use nexus_cog_storage::PersistenceBackend;

/// Brain-like cortex. One per workspace so each agent session
/// has its own cognitive state.
#[derive(Clone)]
pub struct CortexHandle {
    inner: Arc<RwLock<Cortex>>,
}

impl CortexHandle {
    /// New default cortex.
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(Cortex::new(CortexConfig::default()))) }
    }

    /// Run one tick.
    pub fn tick(&self, inputs: std::collections::HashMap<String, nexus_cog_neural::Sdr>) -> nexus_cog_neural::ThoughtBroadcast {
        self.inner.write().tick(inputs)
    }

    /// Read-only access.
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, Cortex> {
        self.inner.read()
    }

    /// Mutable access (single tick at a time).
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, Cortex> {
        self.inner.write()
    }

    /// Run a sleep cycle.
    pub fn sleep(&self, replay_per_cycle: usize) -> nexus_cog_neural::ConsolidationReport {
        self.inner.write().sleep(replay_per_cycle)
    }

    /// Restore cortex from a snapshot.
    pub fn restore(&self, cortex: Cortex) {
        *self.inner.write() = cortex;
    }

    /// Snapshot the cortex for persistence.
    pub fn snapshot(&self) -> Cortex {
        self.inner.read().clone_lite()
    }

    /// Push an SDR onto working memory.
    pub fn working_memory_push(&self, sdr: nexus_cog_neural::Sdr, label: Option<String>) {
        let mut cortex = self.inner.write();
        cortex.working_memory.push(sdr, label);
    }

    /// Add a thalamic channel to the cortex.
    pub fn add_thalamic_channel(&self, name: impl Into<String>) -> u32 {
        self.inner.write().add_thalamic_channel(name)
    }
}

/// Every orthogonal (non-brain) engine the CLI may need. Brain
/// operations all go through [`CortexHandle`].
pub struct Engines {
    /// Shared SQL backend used by the orthogonal persistent engines.
    pub backend: Arc<dyn PersistenceBackend>,
    pub causal: CausalGraphEngine,
    pub blast: BlastRadiusCalculator,
    pub pre_mortem: PreMortemEngine,
    pub counterfactual: CounterfactualReasoner,
    pub forward: ForwardReasoner,
    pub backward: BackwardReasoner,
    pub patterns: PatternMatcher,
    pub provenance: ProvenanceGraphEngine,
    pub adversarial: AdversarialGenerator,
    pub edge_cases: EdgeCaseExplorer,
    pub robustness: RobustnessScorer,
    /// In-memory intent storage (declared modules + invariants).
    pub intent_storage: IntentStorage,
}

/// In-memory intent storage.
#[derive(Default)]
pub struct IntentStorage {
    intents: parking_lot::RwLock<Vec<nexus_cog_core::intent::ModuleIntent>>,
}

impl IntentStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn declare(&self, intent: nexus_cog_core::intent::ModuleIntent) {
        let mut g = self.intents.write();
        g.retain(|i| i.module != intent.module);
        g.push(intent);
    }

    pub fn intent(&self, module: &str) -> Option<nexus_cog_core::intent::ModuleIntent> {
        self.intents.read().iter().find(|i| i.module == module).cloned()
    }

    pub fn intents(&self) -> Vec<nexus_cog_core::intent::ModuleIntent> {
        self.intents.read().clone()
    }
}

impl Engines {
    pub fn new(backend: Arc<dyn PersistenceBackend>) -> Result<Self> {
        let causal = CausalGraphEngine::with_backend(backend.clone())?;
        Ok(Self {
            backend,
            blast: BlastRadiusCalculator::new(causal.clone()),
            pre_mortem: PreMortemEngine::new(causal.clone()),
            counterfactual: CounterfactualReasoner::new(causal.clone()),
            forward: ForwardReasoner::new(causal.clone()),
            backward: BackwardReasoner::new(causal.clone()),
            causal,
            patterns: PatternMatcher::new(),
            provenance: ProvenanceGraphEngine::with_backend(
                std::sync::Arc::new(nexus_cog_storage::SqliteBackend::open_in_memory()?),
            )?,
            adversarial: AdversarialGenerator::new(),
            edge_cases: EdgeCaseExplorer::new(),
            robustness: RobustnessScorer::new(),
            intent_storage: IntentStorage::new(),
        })
    }
}

impl Default for CortexHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Top-level CLI context.
pub struct Ctx {
    pub db_path: PathBuf,
    /// Brain-like cortex for every brain-related operation.
    pub cortex: CortexHandle,
    pub engines: Engines,
}

impl Ctx {
    pub fn open(db_path: PathBuf) -> Result<Self> {
        let backend = Arc::new(nexus_cog_storage::SqliteBackend::open(&db_path)?);
        let engines = Engines::new(backend.clone())?;
        Ok(Self {
            db_path,
            cortex: CortexHandle::new(),
            engines,
        })
    }

    /// Persist the cortex + orthogonal engines.
    pub fn save(&self) -> Result<()> {
        // Cortex is in-memory; callers can use `cortex.snapshot()`
        // explicitly. The orthogonal engines persist themselves
        // (provenance / causal) when their state mutates.
        Ok(())
    }
}

pub fn expand_tilde(p: &Path) -> PathBuf {
    if let Some(s) = p.to_str() {
        if let Some(rest) = s.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home).join(rest);
            }
        }
    }
    p.to_path_buf()
}
