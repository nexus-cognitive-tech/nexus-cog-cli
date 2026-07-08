//! Shared CLI context — owns every engine handle.

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
use nexus_cog_intent::{IntentChecker, IntentDeclarator, IntentDriftTracker};
use nexus_cog_palace::{PersistentPalace, SqliteBackend};
use nexus_cog_patterns::PatternMatcher;
use nexus_cog_provenance::ProvenanceGraphEngine;

/// Every engine handle the CLI may need.
pub struct Engines {
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
    pub declarator: IntentDeclarator,
    pub intent_checker: IntentChecker,
    pub drift: IntentDriftTracker,
    pub provenance: ProvenanceGraphEngine,
    pub adversarial: AdversarialGenerator,
    pub edge_cases: EdgeCaseExplorer,
    pub robustness: RobustnessScorer,
}

impl Engines {
    pub fn new() -> Self {
        let causal = CausalGraphEngine::new();
        Self {
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
            ltm: LongTermMemory::new(),
            learner: AdaptiveLearner::new(),
            predictor: SuccessPredictor::new(),
            declarator: IntentDeclarator::new(),
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
        let palace = PersistentPalace::new(backend, &palace_id);
        palace.load().context("palace load")?;
        Ok(Self {
            db_path,
            palace_id,
            palace,
            engines: Engines::new(),
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
