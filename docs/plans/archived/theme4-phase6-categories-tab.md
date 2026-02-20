---
status: pending
date: 2026-02-05
phase: 6
theme: 4
title: "Categories Tab"
dependencies: [phase2-classification, phase5-insights-core]
parallelizable_with: [phase7-trends-tab, phase8-benchmarks-tab]
---

# Phase 6: Categories Tab

> **Goal:** Visualize session categories with interactive treemap, enable drill-down exploration, and surface category-specific insights.

## Overview

The Categories Tab is the second tab on the `/insights` page. It provides a visual breakdown of how users spend their AI coding time across the classification hierarchy (Code Work, Support Work, Thinking Work). Users can drill down from L1 categories to L2 subcategories to L3 specifics, viewing performance metrics and AI-generated recommendations at each level.

**Key Features:**
- Interactive treemap visualization (default view)
- Multiple view modes: Treemap, Sunburst, Bar Chart, Table
- Category drill-down with breadcrumb navigation
- Category stats panel with metrics comparison
- AI-generated insights per category

**Dependencies:**
- Phase 2 (Classification System) — Needs `category_l1`, `category_l2`, `category_l3` populated on sessions
- Phase 5 (Insights Core) — Needs `/insights` page layout, tab infrastructure, time range filter

---

## Classification Hierarchy Reference

```
Code Work (L1)
├── Feature (L2)
│   ├── new-component (L3)
│   ├── add-functionality (L3)
│   └── integration (L3)
├── Bug Fix (L2)
│   ├── error-fix (L3)
│   ├── logic-fix (L3)
│   └── performance-fix (L3)
├── Refactor (L2)
│   ├── cleanup (L3)
│   ├── pattern-migration (L3)
│   └── dependency-update (L3)
└── Testing (L2)
    ├── unit-tests (L3)
    ├── integration-tests (L3)
    └── test-fixes (L3)

Support Work (L1)
├── Docs (L2)
│   ├── code-comments (L3)
│   ├── readme-guides (L3)
│   └── api-docs (L3)
├── Config (L2)
│   ├── env-setup (L3)
│   ├── build-tooling (L3)
│   └── dependencies (L3)
└── Ops (L2)
    ├── ci-cd (L3)
    ├── deployment (L3)
    └── monitoring (L3)

Thinking Work (L1)
├── Planning (L2)
│   ├── brainstorming (L3)
│   ├── design-doc (L3)
│   └── task-breakdown (L3)
├── Explanation (L2)
│   ├── code-understanding (L3)
│   ├── concept-learning (L3)
│   └── debug-investigation (L3)
└── Architecture (L2)
    ├── system-design (L3)
    ├── data-modeling (L3)
    └── api-design (L3)
```

---

## Tasks

### 6.1 GET /api/insights/categories Endpoint

**Description:** Backend endpoint returning hierarchical category data with aggregated metrics.

#### 6.1.1 Define Response Types

**File:** `crates/server/src/routes/insights.rs`

```rust
use serde::Serialize;
use ts_rs::TS;

/// Top-level category breakdown percentages
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategoryBreakdown {
    pub code_work: CategorySummary,
    pub support_work: CategorySummary,
    pub thinking_work: CategorySummary,
    pub uncategorized: CategorySummary,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategorySummary {
    pub count: u32,
    pub percentage: f64,
}

/// Hierarchical category node for treemap
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategoryNode {
    /// Hierarchical ID: 'code_work', 'code_work/feature', 'code_work/feature/new-component'
    pub id: String,
    /// Category level: 1, 2, or 3
    pub level: u8,
    /// Display name
    pub name: String,
    /// Number of sessions
    pub count: u32,
    /// Percentage of total sessions
    pub percentage: f64,
    /// Average re-edit rate (files re-edited / files edited)
    pub avg_reedit_rate: f64,
    /// Average session duration in seconds
    pub avg_duration: u32,
    /// Average prompts per session
    pub avg_prompts: f64,
    /// Percentage of sessions with commits
    pub commit_rate: f64,
    /// AI-generated insight/recommendation (nullable)
    pub insight: Option<String>,
    /// Child categories (empty for L3)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CategoryNode>,
}

/// Full categories response
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct CategoriesResponse {
    /// High-level breakdown percentages
    pub breakdown: CategoryBreakdown,
    /// Hierarchical category tree
    pub categories: Vec<CategoryNode>,
    /// User's overall averages for comparison
    pub overall_averages: OverallAverages,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct OverallAverages {
    pub avg_reedit_rate: f64,
    pub avg_duration: u32,
    pub avg_prompts: f64,
    pub commit_rate: f64,
}
```

#### 6.1.2 Implement Database Queries

**File:** `crates/db/src/categories.rs` (new file)

```rust
use rusqlite::params;
use crate::Database;

/// Row returned from category aggregation query
pub struct CategoryCount {
    pub category_l1: String,
    pub category_l2: Option<String>,
    pub category_l3: Option<String>,
    pub count: u32,
    pub avg_reedit_rate: f64,
    pub avg_duration: u32,
    pub avg_prompts: f64,
    pub commit_rate: f64,
}

impl Database {
    /// Get category counts grouped by L1/L2/L3
    pub async fn get_category_counts(
        &self,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<Vec<CategoryCount>, Error> {
        let conn = self.conn.lock().await;

        let sql = r#"
            SELECT
                category_l1,
                category_l2,
                category_l3,
                COUNT(*) as count,
                AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate,
                AVG(duration_seconds) as avg_duration,
                AVG(user_prompt_count) as avg_prompts,
                SUM(CASE WHEN commit_count > 0 THEN 1 ELSE 0 END) * 100.0 / COUNT(*) as commit_rate
            FROM sessions
            WHERE category_l1 IS NOT NULL
              AND ($1 IS NULL OR modified_at >= $1)
              AND ($2 IS NULL OR modified_at <= $2)
            GROUP BY category_l1, category_l2, category_l3
            ORDER BY count DESC
        "#;

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![from, to], |row| {
            Ok(CategoryCount {
                category_l1: row.get(0)?,
                category_l2: row.get(1)?,
                category_l3: row.get(2)?,
                count: row.get(3)?,
                avg_reedit_rate: row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                avg_duration: row.get::<_, Option<i64>>(5)?.unwrap_or(0) as u32,
                avg_prompts: row.get::<_, Option<f64>>(6)?.unwrap_or(0.0),
                commit_rate: row.get::<_, Option<f64>>(7)?.unwrap_or(0.0),
            })
        })?;

        rows.collect()
    }

    /// Get uncategorized session count
    pub async fn get_uncategorized_count(
        &self,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<u32, Error> {
        let conn = self.conn.lock().await;

        let sql = r#"
            SELECT COUNT(*) FROM sessions
            WHERE category_l1 IS NULL
              AND ($1 IS NULL OR modified_at >= $1)
              AND ($2 IS NULL OR modified_at <= $2)
        "#;

        conn.query_row(sql, params![from, to], |row| row.get(0))
    }

    /// Get overall averages for comparison
    pub async fn get_overall_averages(
        &self,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<OverallAverages, Error> {
        let conn = self.conn.lock().await;

        let sql = r#"
            SELECT
                AVG(CAST(reedited_files_count AS REAL) / NULLIF(files_edited_count, 0)) as avg_reedit_rate,
                AVG(duration_seconds) as avg_duration,
                AVG(user_prompt_count) as avg_prompts,
                SUM(CASE WHEN commit_count > 0 THEN 1 ELSE 0 END) * 100.0 / COUNT(*) as commit_rate
            FROM sessions
            WHERE ($1 IS NULL OR modified_at >= $1)
              AND ($2 IS NULL OR modified_at <= $2)
        "#;

        conn.query_row(sql, params![from, to], |row| {
            Ok(OverallAverages {
                avg_reedit_rate: row.get::<_, Option<f64>>(0)?.unwrap_or(0.0),
                avg_duration: row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u32,
                avg_prompts: row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                commit_rate: row.get::<_, Option<f64>>(3)?.unwrap_or(0.0),
            })
        })
    }
}
```

#### 6.1.3 Implement Route Handler

**File:** `crates/server/src/routes/insights.rs`

```rust
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use std::collections::HashMap;

use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CategoriesQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
}

/// GET /api/insights/categories
pub async fn get_categories(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CategoriesQuery>,
) -> ApiResult<Json<CategoriesResponse>> {
    let (from, to) = validate_time_range(query.from, query.to)?;

    // Get raw category counts
    let counts = state.db.get_category_counts(from, to).await?;
    let uncategorized = state.db.get_uncategorized_count(from, to).await?;
    let overall = state.db.get_overall_averages(from, to).await?;

    // Calculate total
    let total: u32 = counts.iter().map(|c| c.count).sum::<u32>() + uncategorized;

    // Build hierarchical tree
    let categories = build_category_tree(&counts, total);

    // Calculate L1 breakdown
    let breakdown = calculate_breakdown(&counts, uncategorized, total);

    // Generate insights for each category (async batch)
    // MVP stub: returns categories unchanged; real insights added in Phase 4 integration
    let categories = generate_category_insights(categories, &overall).await;

    Ok(Json(CategoriesResponse {
        breakdown,
        categories,
        overall_averages: overall,
    }))
}

fn build_category_tree(counts: &[CategoryCount], total: u32) -> Vec<CategoryNode> {
    // Group by L1
    let mut l1_map: HashMap<String, Vec<&CategoryCount>> = HashMap::new();
    for count in counts {
        l1_map.entry(count.category_l1.clone())
            .or_default()
            .push(count);
    }

    let mut result = Vec::new();

    for (l1_name, l1_counts) in l1_map {
        let l1_total: u32 = l1_counts.iter().map(|c| c.count).sum();

        // Group by L2 within L1
        let mut l2_map: HashMap<String, Vec<&CategoryCount>> = HashMap::new();
        for count in &l1_counts {
            if let Some(l2) = &count.category_l2 {
                l2_map.entry(l2.clone()).or_default().push(*count);
            }
        }

        let mut l2_children = Vec::new();
        for (l2_name, l2_counts) in l2_map {
            let l2_total: u32 = l2_counts.iter().map(|c| c.count).sum();

            // Build L3 children
            let l3_children: Vec<CategoryNode> = l2_counts
                .iter()
                .filter_map(|c| {
                    c.category_l3.as_ref().map(|l3| CategoryNode {
                        id: format!("{}/{}/{}", &l1_name, &l2_name, l3),
                        level: 3,
                        name: format_category_name(l3),
                        count: c.count,
                        percentage: (c.count as f64 / total as f64) * 100.0,
                        avg_reedit_rate: c.avg_reedit_rate,
                        avg_duration: c.avg_duration,
                        avg_prompts: c.avg_prompts,
                        commit_rate: c.commit_rate,
                        insight: None,
                        children: vec![],
                    })
                })
                .collect();

            // Calculate L2 aggregates
            let (avg_reedit, avg_dur, avg_prompts, commit_rate) =
                aggregate_metrics(&l2_counts);

            l2_children.push(CategoryNode {
                id: format!("{}/{}", &l1_name, &l2_name),
                level: 2,
                name: format_category_name(&l2_name),
                count: l2_total,
                percentage: (l2_total as f64 / total as f64) * 100.0,
                avg_reedit_rate: avg_reedit,
                avg_duration: avg_dur,
                avg_prompts: avg_prompts,
                commit_rate: commit_rate,
                insight: None,
                children: l3_children,
            });
        }

        // Sort L2 by count descending
        l2_children.sort_by(|a, b| b.count.cmp(&a.count));

        // Calculate L1 aggregates
        let (avg_reedit, avg_dur, avg_prompts, commit_rate) =
            aggregate_metrics(&l1_counts);

        result.push(CategoryNode {
            id: l1_name.clone(),
            level: 1,
            name: format_category_name(&l1_name),
            count: l1_total,
            percentage: (l1_total as f64 / total as f64) * 100.0,
            avg_reedit_rate: avg_reedit,
            avg_duration: avg_dur,
            avg_prompts: avg_prompts,
            commit_rate: commit_rate,
            insight: None,
            children: l2_children,
        });
    }

    // Sort L1 by count descending
    result.sort_by(|a, b| b.count.cmp(&a.count));
    result
}

/// MVP stub: returns categories unchanged.
/// Real insight generation will be added when Phase 4 Pattern Engine is integrated.
async fn generate_category_insights(
    categories: Vec<CategoryNode>,
    _overall: &OverallAverages,
) -> Vec<CategoryNode> {
    // TODO: Phase 4 integration will populate `insight` field based on:
    // - Comparison to overall averages
    // - Trend analysis
    // - Pattern matching from Pattern Engine
    categories
}

fn format_category_name(slug: &str) -> String {
    slug.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn aggregate_metrics(counts: &[&CategoryCount]) -> (f64, u32, f64, f64) {
    let total: u32 = counts.iter().map(|c| c.count).sum();
    if total == 0 {
        return (0.0, 0, 0.0, 0.0);
    }

    let weighted_reedit: f64 = counts.iter()
        .map(|c| c.avg_reedit_rate * c.count as f64)
        .sum::<f64>() / total as f64;

    let weighted_dur: f64 = counts.iter()
        .map(|c| c.avg_duration as f64 * c.count as f64)
        .sum::<f64>() / total as f64;

    let weighted_prompts: f64 = counts.iter()
        .map(|c| c.avg_prompts * c.count as f64)
        .sum::<f64>() / total as f64;

    let weighted_commit: f64 = counts.iter()
        .map(|c| c.commit_rate * c.count as f64)
        .sum::<f64>() / total as f64;

    (weighted_reedit, weighted_dur as u32, weighted_prompts, weighted_commit)
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/insights/categories", get(get_categories))
}
```

#### 6.1.4 Add Route to Server

**File:** `crates/server/src/routes/mod.rs`

```rust
pub mod insights;

// In create_router():
.merge(insights::router())
```

---

### 6.2 Treemap Visualization

**Description:** Interactive treemap showing category proportions with drill-down capability.

#### 6.2.1 Chart Library Selection

**Recommendation: Recharts**

| Library | Pros | Cons | Recommendation |
|---------|------|------|----------------|
| **Recharts** | React-native, composable, good treemap | Limited customization | **Use for MVP** |
| **D3** | Full control, any visualization | Steep learning curve, manual React integration | Use if Recharts insufficient |
| **Nivo** | Beautiful defaults, React-native | Heavier bundle, less flexible | Alternative |
| **Visx** | Low-level D3+React | More setup work | Skip |

**Install:**
```bash
pnpm add recharts @types/recharts
```

#### 6.2.2 Treemap Component

**File:** `src/components/insights/CategoryTreemap.tsx`

```tsx
import { useState, useMemo, useCallback } from 'react'
import { Treemap, ResponsiveContainer, Tooltip } from 'recharts'
import type { CategoryNode } from '@/types/generated/CategoryNode'

// Color mapping for L1 categories
const CATEGORY_COLORS = {
  code_work: { fill: '#3B82F6', hover: '#2563EB' },      // Blue
  support_work: { fill: '#10B981', hover: '#059669' },   // Green
  thinking_work: { fill: '#8B5CF6', hover: '#7C3AED' },  // Purple
  uncategorized: { fill: '#6B7280', hover: '#4B5563' },  // Gray
} as const

interface TreemapProps {
  data: CategoryNode[]
  onCategoryClick: (categoryId: string) => void
  selectedCategory: string | null
}

export function CategoryTreemap({ data, onCategoryClick, selectedCategory }: TreemapProps) {
  const [hoveredId, setHoveredId] = useState<string | null>(null)

  // Transform data for Recharts treemap format
  const treemapData = useMemo(() => {
    return data.map(l1 => ({
      name: l1.name,
      id: l1.id,
      size: l1.count,
      percentage: l1.percentage,
      avgReeditRate: l1.avgReeditRate,
      avgDuration: l1.avgDuration,
      fill: CATEGORY_COLORS[l1.id as keyof typeof CATEGORY_COLORS]?.fill ?? '#6B7280',
      children: l1.children.map(l2 => ({
        name: l2.name,
        id: l2.id,
        size: l2.count,
        percentage: l2.percentage,
        avgReeditRate: l2.avgReeditRate,
        avgDuration: l2.avgDuration,
        parentId: l1.id,
        fill: CATEGORY_COLORS[l1.id as keyof typeof CATEGORY_COLORS]?.fill ?? '#6B7280',
      })),
    }))
  }, [data])

  const handleClick = useCallback((node: any) => {
    if (node?.id) {
      onCategoryClick(node.id)
    }
  }, [onCategoryClick])

  const CustomContent = useCallback(({ x, y, width, height, name, percentage, id }: any) => {
    if (width < 50 || height < 30) return null

    const isHovered = hoveredId === id
    const isSelected = selectedCategory === id

    return (
      <g>
        <rect
          x={x}
          y={y}
          width={width}
          height={height}
          style={{
            fill: CATEGORY_COLORS[id.split('/')[0] as keyof typeof CATEGORY_COLORS]?.fill ?? '#6B7280',
            stroke: isSelected ? '#FFF' : isHovered ? '#FFF' : 'none',
            strokeWidth: isSelected ? 3 : isHovered ? 2 : 0,
            opacity: isHovered || isSelected ? 1 : 0.85,
            cursor: 'pointer',
            transition: 'all 150ms ease-out',
          }}
          onClick={() => handleClick({ id })}
          onMouseEnter={() => setHoveredId(id)}
          onMouseLeave={() => setHoveredId(null)}
        />
        <text
          x={x + width / 2}
          y={y + height / 2 - 8}
          textAnchor="middle"
          fill="#FFF"
          fontSize={width > 100 ? 14 : 12}
          fontWeight={600}
          style={{ pointerEvents: 'none' }}
        >
          {name}
        </text>
        <text
          x={x + width / 2}
          y={y + height / 2 + 10}
          textAnchor="middle"
          fill="#FFF"
          fontSize={12}
          opacity={0.8}
          style={{ pointerEvents: 'none' }}
        >
          {percentage.toFixed(0)}%
        </text>
      </g>
    )
  }, [hoveredId, selectedCategory, handleClick])

  return (
    <div className="w-full h-[400px]">
      <ResponsiveContainer width="100%" height="100%">
        <Treemap
          data={treemapData}
          dataKey="size"
          aspectRatio={4 / 3}
          stroke="#1F2937"
          content={<CustomContent />}
        >
          <Tooltip
            content={({ payload }) => {
              if (!payload?.[0]) return null
              const data = payload[0].payload
              return (
                <div className="bg-gray-900 text-white px-3 py-2 rounded-lg shadow-lg text-sm">
                  <div className="font-semibold">{data.name}</div>
                  <div className="text-gray-300">{data.size} sessions ({data.percentage.toFixed(1)}%)</div>
                  <div className="text-gray-400 text-xs mt-1">Click to drill down</div>
                </div>
              )
            }}
          />
        </Treemap>
      </ResponsiveContainer>
    </div>
  )
}
```

#### 6.2.3 Treemap Container with View Toggle

**File:** `src/components/insights/CategoriesVisualization.tsx`

```tsx
import { useState } from 'react'
import { LayoutGrid, PieChart, BarChart3, Table } from 'lucide-react'
import { CategoryTreemap } from './CategoryTreemap'
import { CategorySunburst } from './CategorySunburst'
import { CategoryBarChart } from './CategoryBarChart'
import { CategoryTable } from './CategoryTable'
import type { CategoryNode } from '@/types/generated/CategoryNode'

type ViewMode = 'treemap' | 'sunburst' | 'bar' | 'table'

interface VisualizationProps {
  data: CategoryNode[]
  onCategoryClick: (categoryId: string) => void
  selectedCategory: string | null
}

const VIEW_OPTIONS: { value: ViewMode; label: string; icon: React.ElementType }[] = [
  { value: 'treemap', label: 'Treemap', icon: LayoutGrid },
  { value: 'sunburst', label: 'Sunburst', icon: PieChart },
  { value: 'bar', label: 'Bar Chart', icon: BarChart3 },
  { value: 'table', label: 'Table', icon: Table },
]

export function CategoriesVisualization({ data, onCategoryClick, selectedCategory }: VisualizationProps) {
  const [viewMode, setViewMode] = useState<ViewMode>('treemap')

  return (
    <div className="space-y-4">
      {/* View Toggle */}
      <div className="flex items-center justify-end gap-1 p-1 bg-gray-100 dark:bg-gray-800 rounded-lg w-fit ml-auto">
        {VIEW_OPTIONS.map(({ value, label, icon: Icon }) => (
          <button
            key={value}
            onClick={() => setViewMode(value)}
            className={`
              flex items-center gap-2 px-3 py-1.5 rounded-md text-sm font-medium
              transition-colors duration-150
              ${viewMode === value
                ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-white shadow-sm'
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
              }
            `}
            aria-pressed={viewMode === value}
          >
            <Icon className="w-4 h-4" />
            <span className="hidden sm:inline">{label}</span>
          </button>
        ))}
      </div>

      {/* Visualization */}
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
        {viewMode === 'treemap' && (
          <CategoryTreemap
            data={data}
            onCategoryClick={onCategoryClick}
            selectedCategory={selectedCategory}
          />
        )}
        {viewMode === 'sunburst' && (
          <CategorySunburst
            data={data}
            onCategoryClick={onCategoryClick}
            selectedCategory={selectedCategory}
          />
        )}
        {viewMode === 'bar' && (
          <CategoryBarChart
            data={data}
            onCategoryClick={onCategoryClick}
            selectedCategory={selectedCategory}
          />
        )}
        {viewMode === 'table' && (
          <CategoryTable
            data={data}
            onCategoryClick={onCategoryClick}
            selectedCategory={selectedCategory}
          />
        )}
      </div>
    </div>
  )
}
```

---

### 6.3 Category Drill-Down

**Description:** Detailed view when user clicks a category, showing subcategories, stats, and recent sessions.

#### 6.3.1 Drill-Down Panel Component

**File:** `src/components/insights/CategoryDrillDown.tsx`

```tsx
import { useMemo } from 'react'
import { ArrowLeft, TrendingUp, TrendingDown, Clock, MessageSquare, GitCommit, RefreshCw } from 'lucide-react'
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell } from 'recharts'
import type { CategoryNode } from '@/types/generated/CategoryNode'
import type { OverallAverages } from '@/types/generated/OverallAverages'
import { useSessionsByCategory } from '@/hooks/useSessionsByCategory'
import { SessionCard } from '../SessionCard'
import { formatDuration, formatPercentage } from '@/lib/format'

interface DrillDownProps {
  category: CategoryNode
  parentCategory?: CategoryNode
  overallAverages: OverallAverages
  onBack: () => void
  onDrillDown: (categoryId: string) => void
}

export function CategoryDrillDown({
  category,
  parentCategory,
  overallAverages,
  onBack,
  onDrillDown,
}: DrillDownProps) {
  // Fetch recent sessions for this category
  const { data: recentSessions, isLoading: sessionsLoading } = useSessionsByCategory(
    category.id,
    { limit: 5 }
  )

  // Build breadcrumb path
  const breadcrumbs = useMemo(() => {
    const parts = category.id.split('/')
    return parts.map((_, index) => ({
      id: parts.slice(0, index + 1).join('/'),
      name: parts[index].split('_').map(w => w.charAt(0).toUpperCase() + w.slice(1)).join(' '),
    }))
  }, [category.id])

  // Subcategories bar chart data
  const subcategoryData = useMemo(() => {
    if (!category.children.length) return []
    return category.children.map(child => ({
      name: child.name,
      id: child.id,
      count: child.count,
      percentage: child.percentage,
    })).sort((a, b) => b.count - a.count)
  }, [category.children])

  // Compare metric to overall average
  const compareToAverage = (value: number, average: number, lowerIsBetter = false) => {
    const diff = value - average
    const percentDiff = average > 0 ? (diff / average) * 100 : 0
    const isGood = lowerIsBetter ? diff < 0 : diff > 0
    return { diff, percentDiff, isGood }
  }

  return (
    <div className="space-y-6">
      {/* Header with Breadcrumb */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <button
            onClick={onBack}
            className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            aria-label="Go back"
          >
            <ArrowLeft className="w-5 h-5" />
          </button>
          <div>
            <nav className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
              <button
                onClick={() => onDrillDown('')}
                className="hover:text-gray-900 dark:hover:text-white"
              >
                All
              </button>
              {breadcrumbs.map((crumb, index) => (
                <span key={crumb.id} className="flex items-center gap-2">
                  <span>/</span>
                  {index === breadcrumbs.length - 1 ? (
                    <span className="text-gray-900 dark:text-white font-medium">{crumb.name}</span>
                  ) : (
                    <button
                      onClick={() => onDrillDown(crumb.id)}
                      className="hover:text-gray-900 dark:hover:text-white"
                    >
                      {crumb.name}
                    </button>
                  )}
                </span>
              ))}
            </nav>
            <h2 className="text-xl font-semibold mt-1">
              {category.name}
              <span className="text-gray-500 dark:text-gray-400 ml-2 font-normal">
                ({category.percentage.toFixed(0)}% · {category.count} sessions)
              </span>
            </h2>
          </div>
        </div>
      </div>

      {/* Subcategories Chart */}
      {subcategoryData.length > 0 && (
        <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
          <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4">
            Subcategories
          </h3>
          <div className="h-[200px]">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={subcategoryData} layout="vertical">
                <XAxis type="number" hide />
                <YAxis
                  type="category"
                  dataKey="name"
                  width={120}
                  tick={{ fontSize: 12 }}
                />
                <Tooltip
                  content={({ payload }) => {
                    if (!payload?.[0]) return null
                    const data = payload[0].payload
                    return (
                      <div className="bg-gray-900 text-white px-3 py-2 rounded-lg shadow-lg text-sm">
                        <div className="font-semibold">{data.name}</div>
                        <div>{data.count} sessions ({data.percentage.toFixed(1)}%)</div>
                      </div>
                    )
                  }}
                />
                <Bar
                  dataKey="percentage"
                  radius={[0, 4, 4, 0]}
                  onClick={(data) => onDrillDown(data.id)}
                  style={{ cursor: 'pointer' }}
                >
                  {subcategoryData.map((_, index) => (
                    <Cell key={index} fill="#3B82F6" />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>
      )}

      {/* Performance Metrics */}
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
        <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4">
          {category.name} Performance
        </h3>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <MetricCard
            icon={RefreshCw}
            label="Avg Re-edit Rate"
            value={formatPercentage(category.avgReeditRate)}
            comparison={compareToAverage(category.avgReeditRate, overallAverages.avgReeditRate, true)}
            compareLabel="vs overall"
          />
          <MetricCard
            icon={Clock}
            label="Avg Session Length"
            value={formatDuration(category.avgDuration)}
            comparison={compareToAverage(category.avgDuration, overallAverages.avgDuration)}
            compareLabel="vs overall"
          />
          <MetricCard
            icon={MessageSquare}
            label="Avg Prompts"
            value={category.avgPrompts.toFixed(1)}
            comparison={compareToAverage(category.avgPrompts, overallAverages.avgPrompts)}
            compareLabel="vs overall"
          />
          <MetricCard
            icon={GitCommit}
            label="Commit Rate"
            value={formatPercentage(category.commitRate)}
            comparison={compareToAverage(category.commitRate, overallAverages.commitRate)}
            compareLabel="vs overall"
          />
        </div>

        {/* AI Insight */}
        {category.insight && (
          <div className="mt-4 p-3 bg-amber-50 dark:bg-amber-900/20 rounded-lg border border-amber-200 dark:border-amber-800">
            <p className="text-sm text-amber-800 dark:text-amber-200">
              <span className="font-medium">Insight:</span> {category.insight}
            </p>
          </div>
        )}
      </div>

      {/* Recent Sessions */}
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 p-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-sm font-medium text-gray-500 dark:text-gray-400">
            Recent {category.name} Sessions
          </h3>
          <button className="text-sm text-blue-600 dark:text-blue-400 hover:underline">
            View All →
          </button>
        </div>

        {sessionsLoading ? (
          <div className="space-y-3">
            {[...Array(3)].map((_, i) => (
              <div key={i} className="h-16 bg-gray-100 dark:bg-gray-800 rounded animate-pulse" />
            ))}
          </div>
        ) : recentSessions?.length ? (
          <div className="space-y-2">
            {recentSessions.map(session => (
              <SessionCard key={session.id} session={session} compact />
            ))}
          </div>
        ) : (
          <p className="text-sm text-gray-500 dark:text-gray-400 text-center py-4">
            No sessions found for this category
          </p>
        )}
      </div>
    </div>
  )
}

// Metric Card Sub-component
interface MetricCardProps {
  icon: React.ElementType
  label: string
  value: string
  comparison: { diff: number; percentDiff: number; isGood: boolean }
  compareLabel: string
}

function MetricCard({ icon: Icon, label, value, comparison, compareLabel }: MetricCardProps) {
  const TrendIcon = comparison.isGood ? TrendingUp : TrendingDown
  const trendColor = comparison.isGood
    ? 'text-green-600 dark:text-green-400'
    : 'text-red-600 dark:text-red-400'

  return (
    <div className="p-3 rounded-lg bg-gray-50 dark:bg-gray-800">
      <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400 mb-1">
        <Icon className="w-4 h-4" />
        <span className="text-xs">{label}</span>
      </div>
      <div className="text-lg font-semibold">{value}</div>
      {Math.abs(comparison.percentDiff) > 0.5 && (
        <div className={`flex items-center gap-1 text-xs ${trendColor}`}>
          <TrendIcon className="w-3 h-3" />
          <span>
            {comparison.percentDiff > 0 ? '+' : ''}
            {comparison.percentDiff.toFixed(0)}% {compareLabel}
          </span>
        </div>
      )}
    </div>
  )
}
```

#### 6.3.2 Sessions by Category Hook

**File:** `src/hooks/useSessionsByCategory.ts`

```typescript
import { useQuery } from '@tanstack/react-query'
import type { SessionInfo } from '@/types/generated/SessionInfo'

interface Options {
  limit?: number
  from?: number
  to?: number
}

export function useSessionsByCategory(categoryId: string, options: Options = {}) {
  const { limit = 10, from, to } = options

  return useQuery({
    queryKey: ['sessions-by-category', categoryId, limit, from, to],
    queryFn: async (): Promise<SessionInfo[]> => {
      const params = new URLSearchParams()
      params.set('category', categoryId)
      params.set('limit', String(limit))
      if (from) params.set('from', String(from))
      if (to) params.set('to', String(to))

      const res = await fetch(`/api/sessions?${params}`)
      if (!res.ok) throw new Error('Failed to fetch sessions')
      return res.json()
    },
    enabled: !!categoryId,
  })
}
```

#### 6.3.3 Backend: Filter Sessions by Category

**File:** `crates/server/src/routes/sessions.rs` (update existing)

Add `category` query parameter to existing sessions endpoint:

```rust
#[derive(Debug, Deserialize)]
pub struct SessionsQuery {
    // ... existing fields
    pub category: Option<String>,  // e.g., "code_work/feature/new-component"
}

// In handler, add filter:
if let Some(category) = &query.category {
    let parts: Vec<&str> = category.split('/').collect();
    match parts.len() {
        1 => sql.push_str(" AND category_l1 = ?"),
        2 => sql.push_str(" AND category_l1 = ? AND category_l2 = ?"),
        3 => sql.push_str(" AND category_l1 = ? AND category_l2 = ? AND category_l3 = ?"),
        _ => {}
    }
}
```

---

### 6.4 Category Stats Panel

**Description:** Summary statistics for each category with comparison to overall averages.

#### 6.4.1 Category Stats Summary Component

**File:** `src/components/insights/CategoryStatsSummary.tsx`

```tsx
import type { CategoryBreakdown, OverallAverages } from '@/types/generated'
import { Code2, FileText, Brain, HelpCircle } from 'lucide-react'

interface Props {
  breakdown: CategoryBreakdown
  overallAverages: OverallAverages
  onCategoryClick: (categoryId: string) => void
}

const CATEGORY_CONFIG = {
  codeWork: {
    id: 'code_work',
    label: 'Code Work',
    icon: Code2,
    color: 'text-blue-600 dark:text-blue-400',
    bgColor: 'bg-blue-100 dark:bg-blue-900/30',
  },
  supportWork: {
    id: 'support_work',
    label: 'Support Work',
    icon: FileText,
    color: 'text-green-600 dark:text-green-400',
    bgColor: 'bg-green-100 dark:bg-green-900/30',
  },
  thinkingWork: {
    id: 'thinking_work',
    label: 'Thinking Work',
    icon: Brain,
    color: 'text-purple-600 dark:text-purple-400',
    bgColor: 'bg-purple-100 dark:bg-purple-900/30',
  },
  uncategorized: {
    id: 'uncategorized',
    label: 'Uncategorized',
    icon: HelpCircle,
    color: 'text-gray-600 dark:text-gray-400',
    bgColor: 'bg-gray-100 dark:bg-gray-800',
  },
}

export function CategoryStatsSummary({ breakdown, onCategoryClick }: Props) {
  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
      {Object.entries(CATEGORY_CONFIG).map(([key, config]) => {
        const data = breakdown[key as keyof CategoryBreakdown]
        const Icon = config.icon

        return (
          <button
            key={key}
            onClick={() => config.id !== 'uncategorized' && onCategoryClick(config.id)}
            disabled={config.id === 'uncategorized'}
            className={`
              p-4 rounded-lg border border-gray-200 dark:border-gray-700
              ${config.id !== 'uncategorized' ? 'hover:border-gray-300 dark:hover:border-gray-600 cursor-pointer' : ''}
              transition-colors text-left
            `}
          >
            <div className={`inline-flex p-2 rounded-lg ${config.bgColor} mb-3`}>
              <Icon className={`w-5 h-5 ${config.color}`} />
            </div>
            <div className="text-2xl font-bold">{data.percentage.toFixed(0)}%</div>
            <div className="text-sm text-gray-500 dark:text-gray-400">{config.label}</div>
            <div className="text-xs text-gray-400 dark:text-gray-500 mt-1">
              {data.count} sessions
            </div>
          </button>
        )
      })}
    </div>
  )
}
```

---

### 6.5 Categories Tab Container

**Description:** Main container component that orchestrates all category tab features.

**File:** `src/components/insights/CategoriesTab.tsx`

```tsx
import { useState, useCallback } from 'react'
import { useQuery } from '@tanstack/react-query'
// useTimeRange is defined in Phase 5 (Insights Core)
// Returns { from, to } timestamps based on user's selected time range filter
import { useTimeRange } from '@/hooks/useTimeRange'
import { CategoryStatsSummary } from './CategoryStatsSummary'
import { CategoriesVisualization } from './CategoriesVisualization'
import { CategoryDrillDown } from './CategoryDrillDown'
import type { CategoriesResponse, CategoryNode } from '@/types/generated'
import { Skeleton } from '@/components/ui/Skeleton'

async function fetchCategories(from?: number, to?: number): Promise<CategoriesResponse> {
  const params = new URLSearchParams()
  if (from) params.set('from', String(from))
  if (to) params.set('to', String(to))

  const res = await fetch(`/api/insights/categories?${params}`)
  if (!res.ok) throw new Error('Failed to fetch categories')
  return res.json()
}

export function CategoriesTab() {
  const { from, to } = useTimeRange()
  const [selectedCategoryId, setSelectedCategoryId] = useState<string | null>(null)

  const { data, isLoading, error } = useQuery({
    queryKey: ['insights-categories', from, to],
    queryFn: () => fetchCategories(from, to),
    staleTime: 60_000, // 1 minute
  })

  // Find selected category in tree
  const findCategory = useCallback((id: string, nodes: CategoryNode[]): CategoryNode | null => {
    for (const node of nodes) {
      if (node.id === id) return node
      if (node.children?.length) {
        const found = findCategory(id, node.children)
        if (found) return found
      }
    }
    return null
  }, [])

  const selectedCategory = selectedCategoryId && data
    ? findCategory(selectedCategoryId, data.categories)
    : null

  // Find parent category for breadcrumb
  const findParent = useCallback((id: string, nodes: CategoryNode[]): CategoryNode | null => {
    const parts = id.split('/')
    if (parts.length <= 1) return null
    const parentId = parts.slice(0, -1).join('/')
    return findCategory(parentId, nodes)
  }, [findCategory])

  const parentCategory = selectedCategoryId && data
    ? findParent(selectedCategoryId, data.categories)
    : null

  const handleCategoryClick = useCallback((categoryId: string) => {
    setSelectedCategoryId(categoryId || null)
  }, [])

  const handleBack = useCallback(() => {
    if (!selectedCategoryId) return
    const parts = selectedCategoryId.split('/')
    if (parts.length <= 1) {
      setSelectedCategoryId(null)
    } else {
      setSelectedCategoryId(parts.slice(0, -1).join('/'))
    }
  }, [selectedCategoryId])

  if (isLoading) {
    return <CategoriesTabSkeleton />
  }

  if (error) {
    return (
      <div className="text-center py-12">
        <p className="text-red-600 dark:text-red-400">Failed to load category data</p>
        <button
          onClick={() => window.location.reload()}
          className="mt-2 text-sm text-blue-600 dark:text-blue-400 hover:underline"
        >
          Retry
        </button>
      </div>
    )
  }

  if (!data) return null

  // Show drill-down view if category selected
  if (selectedCategory) {
    return (
      <CategoryDrillDown
        category={selectedCategory}
        parentCategory={parentCategory ?? undefined}
        overallAverages={data.overallAverages}
        onBack={handleBack}
        onDrillDown={handleCategoryClick}
      />
    )
  }

  // Show overview
  return (
    <div className="space-y-6">
      {/* Quick Stats */}
      <CategoryStatsSummary
        breakdown={data.breakdown}
        overallAverages={data.overallAverages}
        onCategoryClick={handleCategoryClick}
      />

      {/* Visualization */}
      <CategoriesVisualization
        data={data.categories}
        onCategoryClick={handleCategoryClick}
        selectedCategory={selectedCategoryId}
      />
    </div>
  )
}

function CategoriesTabSkeleton() {
  return (
    <div className="space-y-6">
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        {[...Array(4)].map((_, i) => (
          <Skeleton key={i} className="h-32 rounded-lg" />
        ))}
      </div>
      <Skeleton className="h-[400px] rounded-lg" />
    </div>
  )
}
```

---

## API Specification

### GET /api/insights/categories

**Description:** Returns hierarchical category breakdown with aggregated metrics.

**Query Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `from` | `number` | No | Unix timestamp, filter sessions >= from |
| `to` | `number` | No | Unix timestamp, filter sessions <= to |

**Response:** `200 OK`

```json
{
  "breakdown": {
    "codeWork": { "count": 524, "percentage": 62.1 },
    "supportWork": { "count": 194, "percentage": 23.0 },
    "thinkingWork": { "count": 107, "percentage": 12.7 },
    "uncategorized": { "count": 19, "percentage": 2.2 }
  },
  "categories": [
    {
      "id": "code_work",
      "level": 1,
      "name": "Code Work",
      "count": 524,
      "percentage": 62.1,
      "avgReeditRate": 0.28,
      "avgDuration": 1420,
      "avgPrompts": 12.4,
      "commitRate": 76.5,
      "insight": "Your Code Work sessions have higher commit rates than average",
      "children": [
        {
          "id": "code_work/feature",
          "level": 2,
          "name": "Feature",
          "count": 237,
          "percentage": 28.1,
          "avgReeditRate": 0.24,
          "avgDuration": 1680,
          "avgPrompts": 14.2,
          "commitRate": 82.1,
          "insight": null,
          "children": [
            {
              "id": "code_work/feature/new-component",
              "level": 3,
              "name": "New Component",
              "count": 89,
              "percentage": 10.6,
              "avgReeditRate": 0.21,
              "avgDuration": 1920,
              "avgPrompts": 16.3,
              "commitRate": 85.4,
              "insight": null,
              "children": []
            }
          ]
        }
      ]
    }
  ],
  "overallAverages": {
    "avgReeditRate": 0.31,
    "avgDuration": 1280,
    "avgPrompts": 11.2,
    "commitRate": 71.3
  }
}
```

**Error Responses:**

| Status | Condition | Body |
|--------|-----------|------|
| `400` | Invalid time range (from > to) | `{ "error": "'from' must be <= 'to'" }` |
| `500` | Database error | `{ "error": "Internal server error" }` |

---

## UI Mockups

### Overview State (Default)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ /insights                                                                   │
│                                                                             │
│ [ Patterns ]  [ Categories ]  [ Trends ]  [ Benchmarks ]                    │
│               ~~~~~~~~~~~~                                                   │
│                                                                             │
│ ┌───────────────┐ ┌───────────────┐ ┌───────────────┐ ┌───────────────┐    │
│ │ 💻            │ │ 📄            │ │ 🧠            │ │ ❓            │    │
│ │   62%         │ │   23%         │ │   13%         │ │   2%          │    │
│ │ Code Work     │ │ Support Work  │ │ Thinking Work │ │ Uncategorized │    │
│ │ 524 sessions  │ │ 194 sessions  │ │ 107 sessions  │ │ 19 sessions   │    │
│ └───────────────┘ └───────────────┘ └───────────────┘ └───────────────┘    │
│                                                                             │
│                                         [ Treemap | Sunburst | Bar | Table ]│
│ ┌───────────────────────────────────────────────────────────────────────┐  │
│ │ ┌─────────────────────────────────────┐ ┌───────────────────────────┐ │  │
│ │ │                                     │ │                           │ │  │
│ │ │          CODE WORK (62%)            │ │   SUPPORT WORK (23%)      │ │  │
│ │ │                                     │ │                           │ │  │
│ │ │  ┌─────────────┐ ┌───────────────┐  │ │  ┌────────┐ ┌─────────┐  │ │  │
│ │ │  │   Feature   │ │   Bug Fix     │  │ │  │  Docs  │ │ Config  │  │ │  │
│ │ │  │    (28%)    │ │    (22%)      │  │ │  │ (12%)  │ │  (8%)   │  │ │  │
│ │ │  └─────────────┘ └───────────────┘  │ │  └────────┘ └─────────┘  │ │  │
│ │ │  ┌────────────────────┐ ┌────────┐  │ │  ┌─────────────────────┐ │ │  │
│ │ │  │     Refactor       │ │Testing │  │ │  │        Ops          │ │ │  │
│ │ │  │      (8%)          │ │ (4%)   │  │ │  │        (3%)         │ │ │  │
│ │ │  └────────────────────┘ └────────┘  │ │  └─────────────────────┘ │ │  │
│ │ └─────────────────────────────────────┘ └───────────────────────────┘ │  │
│ │ ┌─────────────────────────────────────────────────────────────────────┐│  │
│ │ │                      THINKING WORK (13%)                            ││  │
│ │ │   Planning (6%)    Explanation (5%)    Architecture (2%)            ││  │
│ │ └─────────────────────────────────────────────────────────────────────┘│  │
│ └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Drill-Down State (Bug Fix Selected)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ /insights                                                                   │
│                                                                             │
│ [ Patterns ]  [ Categories ]  [ Trends ]  [ Benchmarks ]                    │
│               ~~~~~~~~~~~~                                                   │
│                                                                             │
│ ← Back                                                                       │
│                                                                             │
│ All / Code Work / Bug Fix                                                   │
│ Bug Fix                                                                     │
│ 22% · 186 sessions                                                          │
│                                                                             │
│ ┌───────────────────────────────────────────────────────────────────────┐  │
│ │ Subcategories                                                         │  │
│ │                                                                       │  │
│ │ error-fix        ████████████████████████████░░░░░░░░  68%  127 sess │  │
│ │ logic-fix        █████████████░░░░░░░░░░░░░░░░░░░░░░░  24%   45 sess │  │
│ │ performance-fix  ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   8%   14 sess │  │
│ └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│ ┌───────────────────────────────────────────────────────────────────────┐  │
│ │ Bug Fix Performance                                                   │  │
│ │                                                                       │  │
│ │ ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐          │  │
│ │ │ 🔄         │ │ ⏱️         │ │ 💬         │ │ 📝         │          │  │
│ │ │ 0.31       │ │ 18 min     │ │ 9.2        │ │ 89%        │          │  │
│ │ │ Re-edit    │ │ Avg Length │ │ Avg Prompts│ │ Commit Rate│          │  │
│ │ │ +11% ↗     │ │ +8% ↗      │ │ -6% ↘      │ │ +18% ↗     │          │  │
│ │ └────────────┘ └────────────┘ └────────────┘ └────────────┘          │  │
│ │                                                                       │  │
│ │ ┌─────────────────────────────────────────────────────────────────┐  │  │
│ │ │ 💡 Your bug fixes commit well but have higher re-edits — try    │  │  │
│ │ │    including error messages in your prompts for better context  │  │  │
│ │ └─────────────────────────────────────────────────────────────────┘  │  │
│ └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│ ┌───────────────────────────────────────────────────────────────────────┐  │
│ │ Recent Bug Fix Sessions                               [ View All → ]  │  │
│ │                                                                       │  │
│ │ ┌─────────────────────────────────────────────────────────────────┐  │  │
│ │ │ "Fix null pointer in auth.ts"    error-fix    23 min   0.12    │  │  │
│ │ └─────────────────────────────────────────────────────────────────┘  │  │
│ │ ┌─────────────────────────────────────────────────────────────────┐  │  │
│ │ │ "Debug pagination issue"         logic-fix    45 min   0.42    │  │  │
│ │ └─────────────────────────────────────────────────────────────────┘  │  │
│ │ ┌─────────────────────────────────────────────────────────────────┐  │  │
│ │ │ "Fix memory leak in cache"       perf-fix     32 min   0.28    │  │  │
│ │ └─────────────────────────────────────────────────────────────────┘  │  │
│ └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Alternative Views

#### Sunburst View

```
┌───────────────────────────────────────────────────────────────┐
│                                                               │
│                    ┌─────────────────┐                        │
│              ┌─────│    Feature      │─────┐                  │
│         ┌────│     └─────────────────┘     │────┐             │
│    ┌────│    │ ┌───────┐   ┌───────┐ │    │    │────┐        │
│    │    │    │ │new-   │   │add-   │ │    │    │    │        │
│    │Code│    │ │comp   │   │func   │ │    │    │Supp│        │
│    │Work│    │ └───────┘   └───────┘ │    │    │ort │        │
│    │    │    └───────────────────────┘    │    │    │        │
│    │    │   Bug Fix    Refactor   Testing │    │    │        │
│    └────│───────────────────────────────────────│────┘        │
│         └───────────────┬───────────────────────┘             │
│                    ┌────┴────┐                                │
│                    │Thinking │                                │
│                    │  Work   │                                │
│                    └─────────┘                                │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

#### Bar Chart View

```
┌───────────────────────────────────────────────────────────────┐
│ Category Distribution                                         │
│                                                               │
│ Code Work > Feature     ████████████████████████████  28.1%  │
│ Code Work > Bug Fix     ███████████████████████       22.0%  │
│ Support > Docs          ████████████                  12.0%  │
│ Support > Config        ████████                       8.0%  │
│ Code Work > Refactor    ████████                       8.0%  │
│ Thinking > Planning     ██████                         6.0%  │
│ Thinking > Explanation  █████                          5.0%  │
│ Code Work > Testing     ████                           4.0%  │
│ Support > Ops           ███                            3.0%  │
│ Thinking > Architecture ██                             2.0%  │
│ Uncategorized           ██                             2.0%  │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

#### Table View

```
┌───────────────────────────────────────────────────────────────────────────┐
│ Category          │ Sessions │   %   │ Re-edit │ Avg Dur │ Commits │     │
├───────────────────┼──────────┼───────┼─────────┼─────────┼─────────┼─────┤
│ ▾ Code Work       │      524 │ 62.1% │   0.28  │  24 min │   76.5% │  ▶  │
│   ▾ Feature       │      237 │ 28.1% │   0.24  │  28 min │   82.1% │  ▶  │
│     new-component │       89 │ 10.6% │   0.21  │  32 min │   85.4% │     │
│     add-function  │       94 │ 11.2% │   0.26  │  26 min │   80.2% │     │
│     integration   │       54 │  6.4% │   0.25  │  24 min │   79.6% │     │
│   ▾ Bug Fix       │      186 │ 22.0% │   0.31  │  18 min │   89.2% │  ▶  │
│   ▸ Refactor      │       67 │  8.0% │   0.22  │  20 min │   71.6% │  ▶  │
│   ▸ Testing       │       34 │  4.0% │   0.19  │  15 min │   58.8% │  ▶  │
│ ▸ Support Work    │      194 │ 23.0% │   0.35  │  16 min │   52.1% │  ▶  │
│ ▸ Thinking Work   │      107 │ 12.7% │   0.42  │  22 min │   28.0% │  ▶  │
│ Uncategorized     │       19 │  2.2% │   0.38  │  14 min │   42.1% │     │
└───────────────────────────────────────────────────────────────────────────┘
```

---

## React Components Summary

| Component | File | Purpose |
|-----------|------|---------|
| `CategoriesTab` | `src/components/insights/CategoriesTab.tsx` | Main container, state management |
| `CategoryStatsSummary` | `src/components/insights/CategoryStatsSummary.tsx` | L1 percentage cards |
| `CategoriesVisualization` | `src/components/insights/CategoriesVisualization.tsx` | View toggle container |
| `CategoryTreemap` | `src/components/insights/CategoryTreemap.tsx` | Treemap chart |
| `CategorySunburst` | `src/components/insights/CategorySunburst.tsx` | Sunburst chart |
| `CategoryBarChart` | `src/components/insights/CategoryBarChart.tsx` | Horizontal bar chart |
| `CategoryTable` | `src/components/insights/CategoryTable.tsx` | Sortable table |
| `CategoryDrillDown` | `src/components/insights/CategoryDrillDown.tsx` | Detail view |
| `useSessionsByCategory` | `src/hooks/useSessionsByCategory.ts` | Data fetching hook |

---

## State Management

### Local State (React useState)

| State | Type | Location | Purpose |
|-------|------|----------|---------|
| `viewMode` | `'treemap' \| 'sunburst' \| 'bar' \| 'table'` | `CategoriesVisualization` | Current visualization |
| `selectedCategoryId` | `string \| null` | `CategoriesTab` | Drill-down navigation |
| `hoveredId` | `string \| null` | `CategoryTreemap` | Hover highlighting |

### URL State (React Router)

| Param | Type | Example | Purpose |
|-------|------|---------|---------|
| `category` | `string` | `?category=code_work/feature` | Deep-link to drill-down |

### Server State (React Query)

| Query Key | Endpoint | Stale Time |
|-----------|----------|------------|
| `['insights-categories', from, to]` | `/api/insights/categories` | 60s |
| `['sessions-by-category', id, limit]` | `/api/sessions?category=X` | 30s |

---

## Testing Strategy

### Backend Tests

**File:** `crates/server/src/routes/insights_test.rs`

| Test | Description |
|------|-------------|
| `test_categories_empty_db` | Returns empty breakdown with zero percentages |
| `test_categories_with_data` | Returns correct hierarchy and percentages |
| `test_categories_time_filter` | Respects from/to query params |
| `test_categories_invalid_range` | Returns 400 for invalid time range |
| `test_category_metrics_aggregation` | Weighted averages calculated correctly |

**File:** `crates/db/src/categories_test.rs`

| Test | Description |
|------|-------------|
| `test_get_category_counts` | Groups by L1/L2/L3 correctly |
| `test_get_uncategorized_count` | Counts NULL category sessions |
| `test_get_overall_averages` | Calculates global averages |

### Frontend Tests

**File:** `src/components/insights/CategoriesTab.test.tsx`

| Test | Description |
|------|-------------|
| `renders loading skeleton` | Shows skeleton while loading |
| `renders error state` | Shows error message on failure |
| `renders category breakdown` | Displays L1 percentage cards |
| `navigates to drill-down on click` | Updates selectedCategoryId |
| `breadcrumb navigation works` | Back button and crumb clicks |

**File:** `src/components/insights/CategoryTreemap.test.tsx`

| Test | Description |
|------|-------------|
| `renders treemap cells` | All L1/L2 visible |
| `handles click events` | onCategoryClick called |
| `shows tooltip on hover` | Tooltip appears with data |
| `applies correct colors` | L1 color mapping |

**File:** `src/components/insights/CategoryDrillDown.test.tsx`

| Test | Description |
|------|-------------|
| `renders category details` | Name, percentage, count |
| `renders subcategory bar chart` | When children exist |
| `renders metrics with comparison` | Arrows for above/below average |
| `renders recent sessions` | Session cards or empty state |
| `back button navigates up` | onBack called |

### E2E Tests

**File:** `e2e/insights-categories.spec.ts`

| Test | Description |
|------|-------------|
| `categories tab loads` | Navigate to /insights, click Categories |
| `treemap interaction` | Click cell, verify drill-down |
| `view toggle works` | Switch between all 4 views |
| `time filter affects data` | Change range, verify API call |
| `deep link works` | Navigate to `?category=code_work/feature` |

---

## Acceptance Criteria

### AC-1: API Endpoint

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 1.1 | `GET /api/insights/categories` | Returns 200 with valid JSON | ☐ |
| 1.2 | Response has `breakdown` | codeWork, supportWork, thinkingWork, uncategorized | ☐ |
| 1.3 | Response has `categories` | Array of CategoryNode | ☐ |
| 1.4 | Response has `overallAverages` | avgReeditRate, avgDuration, avgPrompts, commitRate | ☐ |
| 1.5 | Time filter works | `?from=X&to=Y` filters data | ☐ |
| 1.6 | Invalid range returns 400 | `from > to` triggers error | ☐ |
| 1.7 | Empty data handled | Returns zero percentages, empty arrays | ☐ |

### AC-2: Treemap Visualization

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 2.1 | Treemap renders | All L1 categories visible | ☐ |
| 2.2 | Colors correct | Code=blue, Support=green, Thinking=purple | ☐ |
| 2.3 | Hover shows tooltip | Category name, count, percentage | ☐ |
| 2.4 | Click triggers drill-down | onCategoryClick called with ID | ☐ |
| 2.5 | Responsive | Adapts to container size | ☐ |

### AC-3: View Toggle

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 3.1 | Treemap is default | Selected on load | ☐ |
| 3.2 | Sunburst works | Renders hierarchical circles | ☐ |
| 3.3 | Bar chart works | Horizontal bars, sorted by count | ☐ |
| 3.4 | Table works | Sortable columns, expandable rows | ☐ |
| 3.5 | Toggle persists | Stays selected after drill-down | ☐ |

### AC-4: Drill-Down

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 4.1 | Click category | Shows drill-down view | ☐ |
| 4.2 | Breadcrumb shows | All / L1 / L2 / L3 | ☐ |
| 4.3 | Back button works | Returns to parent level | ☐ |
| 4.4 | Subcategories chart | Bar chart of children | ☐ |
| 4.5 | Metrics panel | 4 metrics with comparison | ☐ |
| 4.6 | Insight shows | AI recommendation if available | ☐ |
| 4.7 | Recent sessions | List of 5 sessions | ☐ |
| 4.8 | View All link | Navigates to filtered session list | ☐ |

### AC-5: Category Stats

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 5.1 | Metrics calculated | Weighted average across sessions | ☐ |
| 5.2 | Comparison arrows | Green up/red down vs overall | ☐ |
| 5.3 | Re-edit rate | Lower is better (inverted) | ☐ |
| 5.4 | Uncategorized shown | Count and percentage | ☐ |

### AC-6: Performance

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 6.1 | API response | < 200ms for 10k sessions | ☐ |
| 6.2 | Treemap render | < 100ms after data load | ☐ |
| 6.3 | View switch | < 50ms transition | ☐ |
| 6.4 | Drill-down | < 100ms state update | ☐ |

### AC-7: Accessibility

| # | Scenario | Expected | Pass |
|---|----------|----------|------|
| 7.1 | Keyboard navigation | Tab through view toggles | ☐ |
| 7.2 | Screen reader | Chart has aria-label | ☐ |
| 7.3 | Color contrast | Meets WCAG AA | ☐ |
| 7.4 | Focus visible | All interactive elements | ☐ |

---

## File Changes Summary

### New Files

| File | Purpose |
|------|---------|
| `crates/db/src/categories.rs` | Database queries for category data |
| `crates/server/src/routes/insights.rs` | API route handler |
| `src/types/generated/CategoriesResponse.ts` | TypeScript types (auto-generated) |
| `src/types/generated/CategoryNode.ts` | TypeScript types (auto-generated) |
| `src/components/insights/CategoriesTab.tsx` | Main container component |
| `src/components/insights/CategoryStatsSummary.tsx` | L1 breakdown cards |
| `src/components/insights/CategoriesVisualization.tsx` | View toggle container |
| `src/components/insights/CategoryTreemap.tsx` | Treemap chart |
| `src/components/insights/CategorySunburst.tsx` | Sunburst chart |
| `src/components/insights/CategoryBarChart.tsx` | Bar chart |
| `src/components/insights/CategoryTable.tsx` | Sortable table |
| `src/components/insights/CategoryDrillDown.tsx` | Detail view |
| `src/hooks/useSessionsByCategory.ts` | Data fetching hook |

### Modified Files

| File | Change |
|------|--------|
| `crates/db/src/lib.rs` | Export categories module |
| `crates/server/src/routes/mod.rs` | Add insights routes |
| `crates/server/src/routes/sessions.rs` | Add `category` filter |
| `src/pages/InsightsPage.tsx` | Add Categories tab |
| `package.json` | Add recharts dependency |

---

## Dependencies

### NPM Packages

```json
{
  "dependencies": {
    "recharts": "^2.12.0"
  },
  "devDependencies": {
    "@types/recharts": "^1.8.0"
  }
}
```

### Rust Crates

No new crates required — uses existing rusqlite, serde, axum.

---

## Notes

- Phase 6 requires Phase 2 (Classification) to be complete so sessions have category data
- Phase 6 requires Phase 5 (Insights Core) for the `/insights` page layout and tab infrastructure
- The `insight` field on CategoryNode will be populated by a background job (Phase 4 Pattern Engine integration)
- Sunburst view is optional for MVP — can be added post-launch if treemap is insufficient
- Table view should support sorting by any column
- Deep links allow sharing specific category views: `/insights?tab=categories&category=code_work/feature`
