#[cfg(test)]
mod tests {
    use crate::utils::branch_filter::BranchFilter;
    use crate::webhooks::gitlab::webhook_handlers::merge_request::handle_merge_request_event;
    use crate::webhooks::gitlab::webhook_handlers::push::handle_push_event;
    use actix_web::web;

    #[test]
    fn test_gitlab_push_event_branch_filtering() {
        let filter = BranchFilter::new(Some("main"), None).unwrap();

        // Mock GitLab push event payload for main branch
        let main_payload = r#"{
            "before": "abc123",
            "after": "def456",
            "ref": "refs/heads/main",
            "project": {
                "name": "test-project",
                "homepage": "https://gitlab.com/test/project"
            },
            "commits": [{
                "message": "Test commit",
                "url": "https://gitlab.com/test/project/-/commit/abc123",
                "author": {"name": "Test User"}
            }],
            "user_name": "testuser"
        }"#;

        // Mock GitLab push event payload for feature branch
        let feature_payload = r#"{
            "before": "abc123",
            "after": "def456",
            "ref": "refs/heads/feature/test",
            "project": {
                "name": "test-project",
                "homepage": "https://gitlab.com/test/project"
            },
            "commits": [{
                "message": "Test commit",
                "url": "https://gitlab.com/test/project/-/commit/abc123",
                "author": {"name": "Test User"}
            }],
            "user_name": "testuser"
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
    fn test_gitlab_push_event_wildcard_filtering() {
        let filter = BranchFilter::new(Some("release/*"), Some("*-wip")).unwrap();

        let release_payload = r#"{
            "before": "abc123",
            "after": "def456",
            "ref": "refs/heads/release/1.0",
            "project": {
                "name": "test-project",
                "homepage": "https://gitlab.com/test/project"
            },
            "commits": [{
                "message": "Test commit",
                "url": "https://gitlab.com/test/project/-/commit/abc123",
                "author": {"name": "Test User"}
            }],
            "user_name": "testuser"
        }"#;

        let release_wip_payload = r#"{
            "before": "abc123",
            "after": "def456",
            "ref": "refs/heads/release/2.0-wip",
            "project": {
                "name": "test-project",
                "homepage": "https://gitlab.com/test/project"
            },
            "commits": [{
                "message": "Test commit",
                "url": "https://gitlab.com/test/project/-/commit/abc123",
                "author": {"name": "Test User"}
            }],
            "user_name": "testuser"
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
    fn test_gitlab_merge_request_event_branch_filtering() {
        let filter = BranchFilter::new(Some("main,develop"), None).unwrap();

        // Merge request targeting main
        let main_mr_payload = r#"{
            "user": {"name": "testuser"},
            "object_attributes": {
                "title": "Test MR",
                "url": "https://gitlab.com/test/project/-/merge_requests/1",
                "source_branch": "feature/new-feature",
                "target_branch": "main",
                "action": "open"
            }
        }"#;

        // Merge request targeting feature branch
        let feature_mr_payload = r#"{
            "user": {"name": "testuser"},
            "object_attributes": {
                "title": "Test MR",
                "url": "https://gitlab.com/test/project/-/merge_requests/2",
                "source_branch": "hotfix/bug-fix",
                "target_branch": "feature/xyz",
                "action": "open"
            }
        }"#;

        let main_mr_bytes = web::Bytes::from(main_mr_payload);
        let feature_mr_bytes = web::Bytes::from(feature_mr_payload);

        // Should process MR to main
        let main_result = handle_merge_request_event(&main_mr_bytes, Some(&filter));
        assert!(!main_result.is_empty());

        // Should filter out MR to feature branch
        let feature_result = handle_merge_request_event(&feature_mr_bytes, Some(&filter));
        assert!(feature_result.is_empty());
    }

    #[test]
    fn test_gitlab_merge_request_exclude_filtering() {
        let filter = BranchFilter::new(None, Some("dependabot/*")).unwrap();

        // Merge request targeting main branch
        let main_mr_payload = r#"{
            "user": {"name": "testuser"},
            "object_attributes": {
                "title": "Regular MR",
                "url": "https://gitlab.com/test/project/-/merge_requests/1",
                "source_branch": "feature/enhancement",
                "target_branch": "main",
                "action": "open"
            }
        }"#;

        // Merge request targeting dependabot branch
        let dependabot_mr_payload = r#"{
            "user": {"name": "dependabot[bot]"},
            "object_attributes": {
                "title": "Bump dependency",
                "url": "https://gitlab.com/test/project/-/merge_requests/2",
                "source_branch": "feature/dependency-update",
                "target_branch": "dependabot/update-lodash",
                "action": "open"
            }
        }"#;

        let main_mr_bytes = web::Bytes::from(main_mr_payload);
        let dependabot_mr_bytes = web::Bytes::from(dependabot_mr_payload);

        // Should process MR to main
        let main_result = handle_merge_request_event(&main_mr_bytes, Some(&filter));
        assert!(!main_result.is_empty());

        // Should filter out MR to dependabot branch
        let dependabot_result = handle_merge_request_event(&dependabot_mr_bytes, Some(&filter));
        assert!(dependabot_result.is_empty());
    }

    #[test]
    fn test_gitlab_push_event_delete_branch() {
        let filter = BranchFilter::new(Some("main"), None).unwrap();

        // Delete main branch (should process)
        let delete_main_payload = r#"{
            "before": "abc123",
            "after": "0000000000000000000000000000000000000000",
            "ref": "refs/heads/main",
            "project": {
                "name": "test-project",
                "homepage": "https://gitlab.com/test/project"
            },
            "commits": [],
            "user_name": "testuser"
        }"#;

        // Delete feature branch (should filter out)
        let delete_feature_payload = r#"{
            "before": "abc123",
            "after": "0000000000000000000000000000000000000000",
            "ref": "refs/heads/feature/test",
            "project": {
                "name": "test-project",
                "homepage": "https://gitlab.com/test/project"
            },
            "commits": [],
            "user_name": "testuser"
        }"#;

        let delete_main_bytes = web::Bytes::from(delete_main_payload);
        let delete_feature_bytes = web::Bytes::from(delete_feature_payload);

        // Should process main branch deletion
        let main_result = handle_push_event(&delete_main_bytes, Some(&filter));
        assert!(!main_result.is_empty());

        // Should filter out feature branch deletion
        let feature_result = handle_push_event(&delete_feature_bytes, Some(&filter));
        assert!(feature_result.is_empty());
    }

    #[test]
    fn test_gitlab_merge_request_all_actions() {
        let filter = BranchFilter::new(Some("main"), None).unwrap();

        let actions = vec!["open", "update", "merge", "close", "reopen"];

        for action in actions {
            let payload = format!(
                r#"{{
                    "user": {{"name": "testuser"}},
                    "object_attributes": {{
                        "title": "Test MR",
                        "url": "https://gitlab.com/test/project/-/merge_requests/1",
                        "source_branch": "feature/test",
                        "target_branch": "main",
                        "action": "{}"
                    }}
                }}"#,
                action
            );

            let bytes = web::Bytes::from(payload);
            let result = handle_merge_request_event(&bytes, Some(&filter));

            // All actions should be processed since target is main
            assert!(!result.is_empty(), "Action {} should be processed", action);
        }
    }

    #[test]
    fn test_gitlab_no_filter_processes_all() {
        let main_payload = r#"{
            "before": "abc123",
            "after": "def456",
            "ref": "refs/heads/main",
            "project": {
                "name": "test-project",
                "homepage": "https://gitlab.com/test/project"
            },
            "commits": [{
                "message": "Test commit",
                "url": "https://gitlab.com/test/project/-/commit/abc123",
                "author": {"name": "Test User"}
            }],
            "user_name": "testuser"
        }"#;

        let feature_payload = r#"{
            "before": "abc123",
            "after": "def456",
            "ref": "refs/heads/feature/test",
            "project": {
                "name": "test-project",
                "homepage": "https://gitlab.com/test/project"
            },
            "commits": [{
                "message": "Test commit",
                "url": "https://gitlab.com/test/project/-/commit/abc123",
                "author": {"name": "Test User"}
            }],
            "user_name": "testuser"
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
