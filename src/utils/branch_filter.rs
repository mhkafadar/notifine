use glob::Pattern;

#[derive(Debug)]
pub struct BranchFilter {
    include_patterns: Vec<Pattern>,
    exclude_patterns: Vec<Pattern>,
}

impl BranchFilter {
    pub fn new(include: Option<&str>, exclude: Option<&str>) -> Result<Self, glob::PatternError> {
        let include_patterns = parse_patterns(include)?;
        let exclude_patterns = parse_patterns(exclude)?;

        Ok(BranchFilter {
            include_patterns,
            exclude_patterns,
        })
    }

    pub fn should_process(&self, branch: &str) -> bool {
        // If no filters are specified, process all branches
        if self.include_patterns.is_empty() && self.exclude_patterns.is_empty() {
            return true;
        }

        // Check exclusions first - if branch matches any exclude pattern, don't process
        for pattern in &self.exclude_patterns {
            if pattern.matches(branch) {
                return false;
            }
        }

        // If we have include patterns, branch must match at least one
        if !self.include_patterns.is_empty() {
            for pattern in &self.include_patterns {
                if pattern.matches(branch) {
                    return true;
                }
            }
            return false;
        }

        // If we only have exclude patterns and branch didn't match any, process it
        true
    }
}

fn parse_patterns(patterns_str: Option<&str>) -> Result<Vec<Pattern>, glob::PatternError> {
    match patterns_str {
        Some(s) if !s.is_empty() => s
            .split(',')
            .map(|p| p.trim())
            .filter(|p| !p.is_empty())
            .map(Pattern::new)
            .collect(),
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_filters() {
        let filter = BranchFilter::new(None, None).unwrap();
        assert!(filter.should_process("main"));
        assert!(filter.should_process("develop"));
        assert!(filter.should_process("feature/xyz"));
    }

    #[test]
    fn test_include_only() {
        let filter = BranchFilter::new(Some("main,develop"), None).unwrap();
        assert!(filter.should_process("main"));
        assert!(filter.should_process("develop"));
        assert!(!filter.should_process("feature/xyz"));
    }

    #[test]
    fn test_exclude_only() {
        let filter = BranchFilter::new(None, Some("feature/*,hotfix/*")).unwrap();
        assert!(filter.should_process("main"));
        assert!(filter.should_process("develop"));
        assert!(!filter.should_process("feature/xyz"));
        assert!(!filter.should_process("hotfix/bug"));
    }

    #[test]
    fn test_include_with_wildcard() {
        let filter = BranchFilter::new(Some("main,release/*"), None).unwrap();
        assert!(filter.should_process("main"));
        assert!(filter.should_process("release/1.0"));
        assert!(filter.should_process("release/2.0-beta"));
        assert!(!filter.should_process("develop"));
        assert!(!filter.should_process("feature/xyz"));
    }

    #[test]
    fn test_exclude_takes_precedence() {
        let filter = BranchFilter::new(Some("release/*"), Some("release/beta-*")).unwrap();
        assert!(filter.should_process("release/1.0"));
        assert!(filter.should_process("release/2.0"));
        assert!(!filter.should_process("release/beta-1"));
        assert!(!filter.should_process("release/beta-2"));
    }

    #[test]
    fn test_complex_patterns() {
        let filter = BranchFilter::new(
            Some("main,develop,release/*,hotfix/*"),
            Some("*-wip,*-temp,release/beta-*"),
        )
        .unwrap();

        assert!(filter.should_process("main"));
        assert!(filter.should_process("develop"));
        assert!(filter.should_process("release/1.0"));
        assert!(filter.should_process("hotfix/urgent"));

        assert!(!filter.should_process("main-wip"));
        assert!(!filter.should_process("develop-temp"));
        assert!(!filter.should_process("release/beta-1"));
        assert!(!filter.should_process("feature/new"));
    }

    #[test]
    fn test_empty_string_handling() {
        let filter = BranchFilter::new(Some(""), Some("")).unwrap();
        assert!(filter.should_process("main"));

        let filter2 = BranchFilter::new(Some("main,,develop"), None).unwrap();
        assert!(filter2.should_process("main"));
        assert!(filter2.should_process("develop"));
        assert!(!filter2.should_process("feature"));
    }
}
