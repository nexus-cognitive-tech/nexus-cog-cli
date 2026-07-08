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
use nexus_cog_causal::CausalGraphEngine;
use nexus_cog_cognitive::{CognitiveMirror, CognitiveScaffold, ResponseAnalyzer, ThoughtChain};
use nexus_cog_intel::{
    AdaptiveLearner, LearnerConfig, LongTermMemory, PredictorConfig, SuccessPredictor,
};
use nexus_cog_intent::{IntentChecker, IntentStorage};
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
    pub blast: nexus_cog_causal::BlastRadiusCalculator,
    pub pre_mortem: nexus_cog_causal::PreMortemEngine,
    pub counterfactual: nexus_cog_causal::CounterfactualReasoner,
    pub forward: nexus_cog_causal::ForwardReasoner,
    pub backward: nexus_cog_causal::BackwardReasoner,
    pub patterns: PatternMatcher,
    pub ltm: LongTermMemory,
    pub learner: AdaptiveLearner,
    pub predictor: SuccessPredictor,
    pub intent_storage: IntentStorage,
    pub intent_checker: IntentChecker,
    pub provenance: ProvenanceGraphEngine,
    pub adversarial: AdversarialGenerator,
    pub edge_cases: EdgeCaseExplorer,
    pub robustness: RobustnessScorer,
}

impl Engines {
    /// Build every engine against the shared backend.
    pub fn new(backend: Arc<dyn PersistenceBackend>) -> Result<Self> {
        let causal = CausalGraphEngine::with_backend(backend.clone())?;
        let ltm = LongTermMemory::with_backend(backend.clone());
        let learner = AdaptiveLearner::with_backend(backend.clone(), LearnerConfig::default())?;
        let predictor = SuccessPredictor::with_backend(backend.clone(), PredictorConfig::default())?;
        let intent_storage = IntentStorage::with_backend(backend.clone())?;
        let provenance = ProvenanceGraphEngine::with_backend(backend.clone())?;
        Ok(Self {
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
            blast: nexus_cog_causal::BlastRadiusCalculator::new(causal.clone()),
            pre_mortem: nexus_cog_causal::PreMortemEngine::new(causal.clone()),
            counterfactual: nexus_cog_causal::CounterfactualReasoner::new(causal.clone()),
            forward: nexus_cog_causal::ForwardReasoner::new(causal.clone()),
            backward: nexus_cog_causal::BackwardReasoner::new(causal.clone()),
            causal,
            patterns: PatternMatcher::new(),
            ltm,
            learner,
            predictor,
            intent_storage,
            intent_checker: IntentChecker::new(),
            provenance,
            adversarial: AdversarialGenerator::new(),
            edge_cases: EdgeCaseExplorer::new(),
            robustness: RobustnessScorer::new(),
        })
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
        let engines = Engines::new(backend)?;
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
