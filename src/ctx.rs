//! Shared CLI context — owns every engine handle plus the single
//! [`PersistenceBackend`] that every engine shares.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};

use nexus_cog_antifragile::{AdversarialGenerator, EdgeCaseExplorer, RobustnessScorer};
use nexus_cog_brain::{
    AutoArchitect, CodeGraphBuilder, CodeVerifier, HypothesisEngine, NeuralSearch,
    RiskAnalyzer, SemanticDiffEngine,
};
use nexus_cog_causal::{
    BlastRadiusCalculator, CausalGraphEngine, CounterfactualReasoner, ForwardReasoner,
    PreMortemEngine,
};
use nexus_cog_cognitive::{CognitiveMirror, CognitiveScaffold, ResponseAnalyzer, ThoughtChain};
use nexus_cog_intel::{AdaptiveLearner, LongTermMemory, SuccessPredictor};
use nexus_cog_intent::{IntentChecker, IntentDriftTracker, IntentStorage};
use nexus_cog_palace::{PersistentPalace, SqliteBackend};
use nexus_cog_patterns::PatternMatcher;
use nexus_cog_provenance::ProvenanceGraphEngine;
use nexus_cog_storage::PersistenceBackend;

/// Every engine handle the CLI may need. Every persistent engine is
/// constructed against the shared [`backend`](Self::backend) so there is
/// exactly one connection / file open per CLI session.
pub struct Engines {
    /// The single SQL backend used by every persistent engine.
    pub backend: Arc<dyn PersistenceBackend>,
    pub verifier: CodeVerifier,
    pub risk: RiskAnalyzer,
    pub search: NeuralSearch,
    pub architect: AutoArchitect,
    pub graph: CodeGraphBuilder,
    pub diff: SemanticDiffEngine,
    pub hypothesis: HypothesisEngine,
    pub cognitive: CognitiveScaffold,
    pub mirror: CognitiveMirror,
    pub thought: ThoughtChain,
    pub response: ResponseAnalyzer,
    pub causal: CausalGraphEngine,
    pub blast: BlastRadiusCalculator,
    pub pre_mortem: PreMortemEngine,
    pub counterfactual: CounterfactualReasoner,
    pub patterns: PatternMatcher,
    pub ltm: LongTermMemory,
    pub learner: AdaptiveLearner,
    pub predictor: SuccessPredictor,
    pub intent_storage: IntentStorage,
    pub intent_checker: IntentChecker,
    pub drift: IntentDriftTracker,
    pub provenance: ProvenanceGraphEngine,
    pub adversarial: AdversarialGenerator,
    pub edge_cases: EdgeCaseExplorer,
    pub robustness: RobustnessScorer,
}

impl Engines {
    /// Build every engine against the shared backend.
    pub fn new(backend: Arc<dyn PersistenceBackend>) -> Self {
        let causal = CausalGraphEngine::new();
        let ltm = LongTermMemory::with_backend(backend.clone());
        Self {
            backend: backend.clone(),
            verifier: CodeVerifier::new(),
            risk: RiskAnalyzer::new(),
            search: NeuralSearch::new(),
            architect: AutoArchitect::new(),
            graph: CodeGraphBuilder::new(),
            diff: SemanticDiffEngine::new(),
            hypothesis: HypothesisEngine::new(),
            cognitive: CognitiveScaffold::new(),
            mirror: CognitiveMirror::new(),
            thought: ThoughtChain::new(),
            response: ResponseAnalyzer::new(),
            causal: causal.clone(),
            blast: BlastRadiusCalculator::new(causal.clone()),
            pre_mortem: PreMortemEngine::new(causal.clone()),
            counterfactual: CounterfactualReasoner::new(causal),
            patterns: PatternMatcher::new(),
            ltm,
            learner: AdaptiveLearner::new(),
            predictor: SuccessPredictor::new(),
            intent_storage: IntentStorage::new(),
            intent_checker: IntentChecker::new(),
            drift: IntentDriftTracker::new(),
            provenance: ProvenanceGraphEngine::new(),
            adversarial: AdversarialGenerator::new(),
            edge_cases: EdgeCaseExplorer::new(),
            robustness: RobustnessScorer::new(),
        }
    }
}

/// Top-level CLI context.
pub struct Ctx {
    pub db_path: PathBuf,
    pub palace_id: String,
    pub palace: PersistentPalace,
    pub engines: Engines,
}

impl Ctx {
    pub fn open(db_path: PathBuf, palace_id: String) -> Result<Self> {
        let backend = Arc::new(SqliteBackend::open(&db_path)?);
        let palace = PersistentPalace::new(backend.clone(), &palace_id);
        palace.load().context("palace load")?;
        let engines = Engines::new(backend);
        Ok(Self {
            db_path,
            palace_id,
            palace,
            engines,
        })
    }

    pub fn save(&self) -> Result<()> {
        self.palace.save()?;
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
