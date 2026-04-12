use crate::syntax::delta_logger::{SemanticDelta, EditType};

pub struct ContextAssembler;

impl ContextAssembler {
    pub fn build_incremental_prompt(deltas: &[SemanticDelta]) -> String {
        let mut prompt = String::new();
        prompt.push_str("### INCREMENTAL CONTEXT UPDATE (PIE Layer 1.3)\n");
        
        if deltas.is_empty() {
            prompt.push_str("No semantic changes detected since last sync.\n");
            return prompt;
        }

        for delta in deltas {
            let status = match delta.edit_type {
                EditType::Modified => "MODIFIED",
                EditType::Added => "ADDED   ",
                EditType::Deleted => "DELETED ",
                EditType::Relocated => "MOVED   ",
            };

            prompt.push_str(&format!(
                "- [{}] {}\n",
                status, delta.node_path
            ));
        }

        prompt.push_str("\nAction: LLM should invalidate KV cache for 'MODIFIED' and 'DELETED' nodes. 'MOVED' nodes can be re-indexed with zero re-computation.");
        
        prompt
    }
}
