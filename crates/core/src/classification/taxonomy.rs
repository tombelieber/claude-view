// crates/core/src/classification/taxonomy.rs
//! Taxonomy enums: L1 (top-level), L2 (second-level), L3 (third-level) categories.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Top-level category (L1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "lowercase")]
pub enum CategoryL1 {
    Code,
    Support,
    Thinking,
}

impl CategoryL1 {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Code => "code_work",
            Self::Support => "support_work",
            Self::Thinking => "thinking_work",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "code_work" | "code" => Some(Self::Code),
            "support_work" | "support" => Some(Self::Support),
            "thinking_work" | "thinking" => Some(Self::Thinking),
            _ => None,
        }
    }
}

/// Second-level category (L2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "lowercase")]
pub enum CategoryL2 {
    Feature,
    Bugfix,
    Refactor,
    Testing,
    Docs,
    Config,
    Ops,
    Planning,
    Explanation,
    Architecture,
}

impl CategoryL2 {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Bugfix => "bug_fix",
            Self::Refactor => "refactor",
            Self::Testing => "testing",
            Self::Docs => "docs",
            Self::Config => "config",
            Self::Ops => "ops",
            Self::Planning => "planning",
            Self::Explanation => "explanation",
            Self::Architecture => "architecture",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "feature" => Some(Self::Feature),
            "bug_fix" | "bugfix" => Some(Self::Bugfix),
            "refactor" => Some(Self::Refactor),
            "testing" => Some(Self::Testing),
            "docs" => Some(Self::Docs),
            "config" => Some(Self::Config),
            "ops" => Some(Self::Ops),
            "planning" => Some(Self::Planning),
            "explanation" => Some(Self::Explanation),
            "architecture" => Some(Self::Architecture),
            _ => None,
        }
    }

    /// Get the parent L1 category for this L2 category.
    pub fn parent_l1(&self) -> CategoryL1 {
        match self {
            Self::Feature | Self::Bugfix | Self::Refactor | Self::Testing => CategoryL1::Code,
            Self::Docs | Self::Config | Self::Ops => CategoryL1::Support,
            Self::Planning | Self::Explanation | Self::Architecture => CategoryL1::Thinking,
        }
    }
}

/// Third-level category (L3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "kebab-case")]
pub enum CategoryL3 {
    // Feature
    NewComponent,
    AddFunctionality,
    Integration,
    // Bugfix
    ErrorFix,
    LogicFix,
    PerformanceFix,
    // Refactor
    Cleanup,
    PatternMigration,
    DependencyUpdate,
    // Testing
    UnitTests,
    IntegrationTests,
    TestFixes,
    // Docs
    CodeComments,
    ReadmeGuides,
    ApiDocs,
    // Config
    EnvSetup,
    BuildTooling,
    Dependencies,
    // Ops
    CiCd,
    Deployment,
    Monitoring,
    // Planning
    Brainstorming,
    DesignDoc,
    TaskBreakdown,
    // Explanation
    CodeUnderstanding,
    ConceptLearning,
    DebugInvestigation,
    // Architecture
    SystemDesign,
    DataModeling,
    ApiDesign,
}

impl CategoryL3 {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NewComponent => "new-component",
            Self::AddFunctionality => "add-functionality",
            Self::Integration => "integration",
            Self::ErrorFix => "error-fix",
            Self::LogicFix => "logic-fix",
            Self::PerformanceFix => "performance-fix",
            Self::Cleanup => "cleanup",
            Self::PatternMigration => "pattern-migration",
            Self::DependencyUpdate => "dependency-update",
            Self::UnitTests => "unit-tests",
            Self::IntegrationTests => "integration-tests",
            Self::TestFixes => "test-fixes",
            Self::CodeComments => "code-comments",
            Self::ReadmeGuides => "readme-guides",
            Self::ApiDocs => "api-docs",
            Self::EnvSetup => "env-setup",
            Self::BuildTooling => "build-tooling",
            Self::Dependencies => "dependencies",
            Self::CiCd => "ci-cd",
            Self::Deployment => "deployment",
            Self::Monitoring => "monitoring",
            Self::Brainstorming => "brainstorming",
            Self::DesignDoc => "design-doc",
            Self::TaskBreakdown => "task-breakdown",
            Self::CodeUnderstanding => "code-understanding",
            Self::ConceptLearning => "concept-learning",
            Self::DebugInvestigation => "debug-investigation",
            Self::SystemDesign => "system-design",
            Self::DataModeling => "data-modeling",
            Self::ApiDesign => "api-design",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "new-component" => Some(Self::NewComponent),
            "add-functionality" => Some(Self::AddFunctionality),
            "integration" => Some(Self::Integration),
            "error-fix" => Some(Self::ErrorFix),
            "logic-fix" => Some(Self::LogicFix),
            "performance-fix" => Some(Self::PerformanceFix),
            "cleanup" => Some(Self::Cleanup),
            "pattern-migration" => Some(Self::PatternMigration),
            "dependency-update" => Some(Self::DependencyUpdate),
            "unit-tests" => Some(Self::UnitTests),
            "integration-tests" => Some(Self::IntegrationTests),
            "test-fixes" => Some(Self::TestFixes),
            "code-comments" => Some(Self::CodeComments),
            "readme-guides" => Some(Self::ReadmeGuides),
            "api-docs" => Some(Self::ApiDocs),
            "env-setup" => Some(Self::EnvSetup),
            "build-tooling" => Some(Self::BuildTooling),
            "dependencies" => Some(Self::Dependencies),
            "ci-cd" => Some(Self::CiCd),
            "deployment" => Some(Self::Deployment),
            "monitoring" => Some(Self::Monitoring),
            "brainstorming" => Some(Self::Brainstorming),
            "design-doc" => Some(Self::DesignDoc),
            "task-breakdown" => Some(Self::TaskBreakdown),
            "code-understanding" => Some(Self::CodeUnderstanding),
            "concept-learning" => Some(Self::ConceptLearning),
            "debug-investigation" => Some(Self::DebugInvestigation),
            "system-design" => Some(Self::SystemDesign),
            "data-modeling" => Some(Self::DataModeling),
            "api-design" => Some(Self::ApiDesign),
            _ => None,
        }
    }

    /// Get the parent L2 category for this L3 category.
    pub fn parent_l2(&self) -> CategoryL2 {
        match self {
            Self::NewComponent | Self::AddFunctionality | Self::Integration => CategoryL2::Feature,
            Self::ErrorFix | Self::LogicFix | Self::PerformanceFix => CategoryL2::Bugfix,
            Self::Cleanup | Self::PatternMigration | Self::DependencyUpdate => CategoryL2::Refactor,
            Self::UnitTests | Self::IntegrationTests | Self::TestFixes => CategoryL2::Testing,
            Self::CodeComments | Self::ReadmeGuides | Self::ApiDocs => CategoryL2::Docs,
            Self::EnvSetup | Self::BuildTooling | Self::Dependencies => CategoryL2::Config,
            Self::CiCd | Self::Deployment | Self::Monitoring => CategoryL2::Ops,
            Self::Brainstorming | Self::DesignDoc | Self::TaskBreakdown => CategoryL2::Planning,
            Self::CodeUnderstanding | Self::ConceptLearning | Self::DebugInvestigation => {
                CategoryL2::Explanation
            }
            Self::SystemDesign | Self::DataModeling | Self::ApiDesign => CategoryL2::Architecture,
        }
    }
}
