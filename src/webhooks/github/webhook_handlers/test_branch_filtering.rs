#[cfg(test)]
mod tests {
    use super::super::{
        handle_create_event, handle_delete_event, handle_pull_request_event, handle_push_event,
        handle_workflow_run_event,
    };
    use crate::utils::branch_filter::BranchFilter;
    use actix_web::web;

    #[test]
    fn test_push_event_branch_filtering() {
        let filter = BranchFilter::new(Some("main"), None).unwrap();

        // Mock push event payload for main branch
        let main_payload = r#"{
            "ref": "refs/heads/main",
            "before": "abc123",
            "after": "def456",
            "forced": false,
            "commits": [{"message": "Test commit", "url": "http://example.com", "author": {"name": "Test User"}}],
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        // Mock push event payload for feature branch
        let feature_payload = r#"{
            "ref": "refs/heads/feature/test",
            "before": "abc123",
            "after": "def456",
            "forced": false,
            "commits": [{"message": "Test commit", "url": "http://example.com", "author": {"name": "Test User"}}],
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        let main_bytes = web::Bytes::from(main_payload);
        let feature_bytes = web::Bytes::from(feature_payload);

        // Should process main branch
        let main_result = handle_push_event(&main_bytes, Some(&filter));
        assert!(!main_result.is_empty());

        // Should filter out feature branch
        let feature_result = handle_push_event(&feature_bytes, Some(&filter));
        assert!(feature_result.is_empty());
    }

    #[test]
    fn test_push_event_wildcard_filtering() {
        let filter = BranchFilter::new(Some("release/*"), Some("*-wip")).unwrap();

        let release_payload = r#"{
            "ref": "refs/heads/release/1.0",
            "before": "abc123",
            "after": "def456", 
            "forced": false,
            "commits": [{"message": "Test commit", "url": "http://example.com", "author": {"name": "Test User"}}],
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        let release_wip_payload = r#"{
            "ref": "refs/heads/release/2.0-wip",
            "before": "abc123",
            "after": "def456",
            "forced": false,
            "commits": [{"message": "Test commit", "url": "http://example.com", "author": {"name": "Test User"}}],
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        let release_bytes = web::Bytes::from(release_payload);
        let release_wip_bytes = web::Bytes::from(release_wip_payload);

        // Should process release/1.0
        let release_result = handle_push_event(&release_bytes, Some(&filter));
        assert!(!release_result.is_empty());

        // Should filter out release/2.0-wip (excluded)
        let release_wip_result = handle_push_event(&release_wip_bytes, Some(&filter));
        assert!(release_wip_result.is_empty());
    }

    #[test]
    fn test_pull_request_event_branch_filtering() {
        let filter = BranchFilter::new(Some("main,develop"), None).unwrap();

        // PR targeting main
        let main_pr_payload = r#"{
            "action": "opened",
            "pull_request": {
                "html_url": "http://example.com/pr/1",
                "number": 1,
                "title": "Test PR",
                "merged": false,
                "head": {"label": "feature:new-feature"},
                "base": {"label": "origin:main"}
            },
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        // PR targeting feature branch
        let feature_pr_payload = r#"{
            "action": "opened",
            "pull_request": {
                "html_url": "http://example.com/pr/2",
                "number": 2,
                "title": "Test PR",
                "merged": false,
                "head": {"label": "hotfix:bug-fix"},
                "base": {"label": "origin:feature/xyz"}
            },
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        let main_pr_bytes = web::Bytes::from(main_pr_payload);
        let feature_pr_bytes = web::Bytes::from(feature_pr_payload);

        // Should process PR to main
        let main_result = handle_pull_request_event(&main_pr_bytes, Some(&filter));
        assert!(!main_result.is_empty());

        // Should filter out PR to feature branch
        let feature_result = handle_pull_request_event(&feature_pr_bytes, Some(&filter));
        assert!(feature_result.is_empty());
    }

    #[test]
    fn test_workflow_run_event_branch_filtering() {
        let filter = BranchFilter::new(None, Some("dependabot/*")).unwrap();

        // Workflow on main branch
        let main_workflow_payload = r#"{
            "action": "completed",
            "workflow_run": {
                "name": "CI",
                "html_url": "http://example.com/run/1",
                "status": "completed",
                "conclusion": "success",
                "head_branch": "main",
                "run_number": 1
            },
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        // Workflow on dependabot branch
        let dependabot_workflow_payload = r#"{
            "action": "completed",
            "workflow_run": {
                "name": "CI",
                "html_url": "http://example.com/run/2", 
                "status": "completed",
                "conclusion": "success",
                "head_branch": "dependabot/npm_and_yarn/lodash-4.17.21",
                "run_number": 2
            },
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "dependabot[bot]"}
        }"#;

        let main_workflow_bytes = web::Bytes::from(main_workflow_payload);
        let dependabot_workflow_bytes = web::Bytes::from(dependabot_workflow_payload);

        // Should process workflow on main
        let main_result = handle_workflow_run_event(&main_workflow_bytes, Some(&filter));
        assert!(!main_result.is_empty());

        // Should filter out dependabot workflow
        let dependabot_result =
            handle_workflow_run_event(&dependabot_workflow_bytes, Some(&filter));
        assert!(dependabot_result.is_empty());
    }

    #[test]
    fn test_create_delete_event_branch_filtering() {
        let filter = BranchFilter::new(Some("main,release/*"), None).unwrap();

        // Create main branch
        let create_main_payload = r#"{
            "ref": "main",
            "ref_type": "branch",
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        // Create feature branch
        let create_feature_payload = r#"{
            "ref": "feature/new-thing",
            "ref_type": "branch",
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        // Create tag (should not be filtered)
        let create_tag_payload = r#"{
            "ref": "v1.0.0",
            "ref_type": "tag",
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        let create_main_bytes = web::Bytes::from(create_main_payload);
        let create_feature_bytes = web::Bytes::from(create_feature_payload);
        let create_tag_bytes = web::Bytes::from(create_tag_payload);

        // Should process main branch creation
        let main_result = handle_create_event(&create_main_bytes, Some(&filter));
        assert!(!main_result.is_empty());

        // Should filter out feature branch creation
        let feature_result = handle_create_event(&create_feature_bytes, Some(&filter));
        assert!(feature_result.is_empty());

        // Should process tag creation (not subject to branch filtering)
        let tag_result = handle_create_event(&create_tag_bytes, Some(&filter));
        assert!(!tag_result.is_empty());

        // Test delete events too
        let delete_main_result = handle_delete_event(&create_main_bytes, Some(&filter));
        assert!(!delete_main_result.is_empty());

        let delete_feature_result = handle_delete_event(&create_feature_bytes, Some(&filter));
        assert!(delete_feature_result.is_empty());
    }

    #[test]
    fn test_no_filter_processes_all() {
        let main_payload = r#"{
            "ref": "refs/heads/main",
            "before": "abc123",
            "after": "def456",
            "forced": false,
            "commits": [{"message": "Test commit", "url": "http://example.com", "author": {"name": "Test User"}}],
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        let feature_payload = r#"{
            "ref": "refs/heads/feature/test",
            "before": "abc123", 
            "after": "def456",
            "forced": false,
            "commits": [{"message": "Test commit", "url": "http://example.com", "author": {"name": "Test User"}}],
            "repository": {"name": "test-repo", "html_url": "http://example.com"},
            "sender": {"login": "testuser"}
        }"#;

        let main_bytes = web::Bytes::from(main_payload);
        let feature_bytes = web::Bytes::from(feature_payload);

        // No filter should process both
        let main_result = handle_push_event(&main_bytes, None);
        assert!(!main_result.is_empty());

        let feature_result = handle_push_event(&feature_bytes, None);
        assert!(!feature_result.is_empty());
    }
}
