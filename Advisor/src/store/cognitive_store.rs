use std::collections::HashMap;

use uuid::Uuid;

use super::clipboard::GlobalClipboard;
use super::embedding::{cosine_similarity, EmbeddingProvider};
use super::memory::{Memory, MemoryResult};
use crate::error::AdvisorError;
use crate::synapse::Synapse;
use crate::thought::{Session, Thought};

/// In-memory cognitive state with search capabilities.
///
/// All state is held in memory. Persistence is handled externally
/// (via Equipment/Pact calls to Vault/BlackBox).
pub struct CognitiveStore {
    pub state: CognitiveStoreState,
    embedding_provider: Option<Box<dyn EmbeddingProvider>>,
}

/// The raw state of the cognitive store.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CognitiveStoreState {
    pub thoughts: HashMap<Uuid, Thought>,
    pub sessions: HashMap<Uuid, Session>,
    pub memories: HashMap<Uuid, Memory>,
    pub clipboard: GlobalClipboard,
    pub synapses: HashMap<Uuid, Synapse>,
}

impl CognitiveStoreState {
    /// Create a new empty cognitive store state with the given clipboard capacity.
    pub fn new(clipboard_max_entries: usize) -> Self {
        Self {
            thoughts: HashMap::new(),
            sessions: HashMap::new(),
            memories: HashMap::new(),
            clipboard: GlobalClipboard::new(clipboard_max_entries),
            synapses: HashMap::new(),
        }
    }
}

impl Default for GlobalClipboard {
    fn default() -> Self {
        Self::new(100)
    }
}

impl CognitiveStore {
    /// Create a new cognitive store with the given clipboard capacity.
    pub fn new(clipboard_max_entries: usize) -> Self {
        Self {
            state: CognitiveStoreState::new(clipboard_max_entries),
            embedding_provider: None,
        }
    }

    pub fn with_embedding_provider(mut self, provider: Box<dyn EmbeddingProvider>) -> Self {
        self.embedding_provider = Some(provider);
        self
    }

    // ── Thoughts ─────────────────────────────────────────────────

    pub fn save_thought(&mut self, thought: Thought) {
        self.state.thoughts.insert(thought.id, thought);
    }

    pub fn get_thought(&self, id: Uuid) -> Option<&Thought> {
        self.state.thoughts.get(&id)
    }

    pub fn thoughts_for_session(&self, session_id: Uuid) -> Vec<&Thought> {
        let mut thoughts: Vec<&Thought> = self
            .state
            .thoughts
            .values()
            .filter(|t| t.session_id == session_id)
            .collect();
        thoughts.sort_by_key(|t| t.created_at);
        thoughts
    }

    pub fn delete_thought(&mut self, id: Uuid) -> bool {
        self.state.thoughts.remove(&id).is_some()
    }

    // ── Sessions ─────────────────────────────────────────────────

    pub fn save_session(&mut self, session: Session) {
        self.state.sessions.insert(session.id, session);
    }

    pub fn get_session(&self, id: Uuid) -> Option<&Session> {
        self.state.sessions.get(&id)
    }

    pub fn get_session_mut(&mut self, id: Uuid) -> Option<&mut Session> {
        self.state.sessions.get_mut(&id)
    }

    pub fn active_sessions(&self) -> Vec<&Session> {
        self.state.sessions.values().filter(|s| s.is_active()).collect()
    }

    // ── Memories ─────────────────────────────────────────────────

    pub fn save_memory(&mut self, mut memory: Memory) {
        // Generate embedding if provider is available
        if memory.embedding.is_none() {
            if let Some(ref provider) = self.embedding_provider {
                memory.embedding = provider.embed(&memory.content);
            }
        }
        self.state.memories.insert(memory.id, memory);
    }

    pub fn get_memory(&self, id: Uuid) -> Option<&Memory> {
        self.state.memories.get(&id)
    }

    /// Search memories by keyword (substring match).
    pub fn search_memories(&mut self, query: &str, max_results: usize) -> Vec<MemoryResult> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<MemoryResult> = Vec::new();

        let matching_ids: Vec<Uuid> = self
            .state
            .memories
            .values()
            .filter(|m| m.content.to_lowercase().contains(&query_lower))
            .map(|m| m.id)
            .collect();

        for id in matching_ids {
            if let Some(memory) = self.state.memories.get_mut(&id) {
                memory.record_access();
                results.push(MemoryResult {
                    content: memory.content.clone(),
                    relevance: 1.0, // keyword match is binary
                    memory_id: memory.id,
                    created_at: memory.created_at,
                });
            }
        }

        results.truncate(max_results);
        results
    }

    /// Search memories by semantic similarity.
    pub fn semantic_search(
        &mut self,
        query: &str,
        min_score: f64,
        max_results: usize,
    ) -> Result<Vec<MemoryResult>, AdvisorError> {
        let provider = self
            .embedding_provider
            .as_ref()
            .ok_or_else(|| AdvisorError::EmbeddingFailed("no embedding provider".into()))?;

        let query_embedding = provider
            .embed(query)
            .ok_or_else(|| AdvisorError::EmbeddingFailed("failed to embed query".into()))?;

        let mut scored: Vec<(Uuid, f64)> = Vec::new();

        for memory in self.state.memories.values() {
            if let Some(ref mem_embedding) = memory.embedding {
                let score = cosine_similarity(&query_embedding, mem_embedding);
                if score >= min_score {
                    scored.push((memory.id, score));
                }
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_results);

        let results = scored
            .into_iter()
            .filter_map(|(id, score)| {
                self.state.memories.get_mut(&id).map(|m| {
                    m.record_access();
                    MemoryResult {
                        content: m.content.clone(),
                        relevance: score,
                        memory_id: m.id,
                        created_at: m.created_at,
                    }
                })
            })
            .collect();

        Ok(results)
    }

    pub fn delete_memory(&mut self, id: Uuid) -> bool {
        self.state.memories.remove(&id).is_some()
    }

    // ── Synapses ─────────────────────────────────────────────────

    pub fn save_synapse(&mut self, synapse: Synapse) {
        self.state.synapses.insert(synapse.id, synapse);
    }

    pub fn get_synapse(&self, id: Uuid) -> Option<&Synapse> {
        self.state.synapses.get(&id)
    }

    pub fn delete_synapse(&mut self, id: Uuid) -> bool {
        self.state.synapses.remove(&id).is_some()
    }

    pub fn prune_weak_synapses(&mut self, min_strength: f64) -> usize {
        let before = self.state.synapses.len();
        self.state.synapses.retain(|_, s| !s.should_prune(min_strength));
        before - self.state.synapses.len()
    }

    // ── Statistics ───────────────────────────────────────────────

    pub fn thought_count(&self) -> usize {
        self.state.thoughts.len()
    }

    pub fn session_count(&self) -> usize {
        self.state.sessions.len()
    }

    pub fn memory_count(&self) -> usize {
        self.state.memories.len()
    }

    pub fn synapse_count(&self) -> usize {
        self.state.synapses.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thought::{ThoughtSource, SessionType};

    #[test]
    fn store_thought_lifecycle() {
        let mut store = CognitiveStore::new(100);
        let session_id = Uuid::new_v4();
        let thought = Thought::new(session_id, "idea", ThoughtSource::Autonomous);
        let tid = thought.id;

        store.save_thought(thought);
        assert_eq!(store.thought_count(), 1);
        assert!(store.get_thought(tid).is_some());

        let session_thoughts = store.thoughts_for_session(session_id);
        assert_eq!(session_thoughts.len(), 1);

        store.delete_thought(tid);
        assert_eq!(store.thought_count(), 0);
    }

    #[test]
    fn store_session_lifecycle() {
        let mut store = CognitiveStore::new(100);
        let session = Session::user("chat");
        let sid = session.id;

        store.save_session(session);
        assert_eq!(store.session_count(), 1);
        assert!(store.get_session(sid).is_some());
        assert_eq!(store.active_sessions().len(), 1);

        store.get_session_mut(sid).unwrap().archive().unwrap();
        assert_eq!(store.active_sessions().len(), 0);
    }

    #[test]
    fn store_memory_keyword_search() {
        let mut store = CognitiveStore::new(100);
        store.save_memory(Memory::new("Rust is a systems language"));
        store.save_memory(Memory::new("Swift is for Apple"));
        store.save_memory(Memory::new("Rust and Swift are both great"));

        let results = store.search_memories("rust", 10);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn store_semantic_search_no_provider() {
        let mut store = CognitiveStore::new(100);
        store.save_memory(Memory::new("test"));
        let result = store.semantic_search("test", 0.5, 10);
        assert!(result.is_err()); // no embedding provider
    }

    #[test]
    fn store_synapse_prune() {
        let mut store = CognitiveStore::new(100);
        let s1 = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.5);
        let mut s2 = Synapse::thought_relates(Uuid::new_v4(), Uuid::new_v4(), 0.15);
        s2.decay(0.05, 0.1);
        store.save_synapse(s1);
        store.save_synapse(s2);

        let pruned = store.prune_weak_synapses(0.1);
        assert_eq!(pruned, 1);
        assert_eq!(store.synapse_count(), 1);
    }

    #[test]
    fn store_home_session() {
        let mut store = CognitiveStore::new(100);
        let home = Session::home();
        assert_eq!(home.session_type, SessionType::Home);
        store.save_session(home);
        assert_eq!(store.session_count(), 1);
    }
}
