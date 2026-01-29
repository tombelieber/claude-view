use vibe_recall_core::{SessionInfo, ToolCounts};

fn main() {
    let session = SessionInfo {
        id: "test".to_string(),
        project: "test".to_string(),
        project_path: "/test".to_string(),
        file_path: "/test/session.jsonl".to_string(),
        modified_at: 1769482232,
        size_bytes: 100,
        preview: "Test".to_string(),
        last_message: "Test".to_string(),
        files_touched: vec![],
        skills_used: vec![],
        tool_counts: ToolCounts::default(),
        message_count: 1,
        turn_count: 1,
        summary: None,
        git_branch: None,
        is_sidechain: false,
        deep_indexed: false,
        total_input_tokens: None,
        total_output_tokens: None,
        total_cache_read_tokens: None,
        total_cache_creation_tokens: None,
        turn_count_api: None,
        primary_model: None,
    };
    
    let json = serde_json::to_string_pretty(&session).unwrap();
    println!("{}", json);
}
