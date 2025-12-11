// ==============================================================================
// IJOKA SEED DATA
// ==============================================================================
// Initial data for development and testing
// ==============================================================================

// Create default project for the ijoka codebase itself
MERGE (p:Project {
  id: "ijoka-core",
  path: "/Users/shakes/DevProjects/ijoka",
  name: "Ijoka",
  description: "Unified observability and orchestration for AI coding agents",
  created_at: datetime(),
  updated_at: datetime(),
  settings: "{}"
});

// Create sample features for the ijoka project
MATCH (p:Project {id: "ijoka-core"})
CREATE (f1:Feature {
  id: randomUUID(),
  description: "Graph database integration with Memgraph",
  category: "infrastructure",
  status: "in_progress",
  priority: 10,
  steps: ["Set up Docker Memgraph", "Create Cypher schema", "Implement Rust client", "Add cache sync"],
  created_at: datetime(),
  updated_at: datetime(),
  work_count: 0
})-[:BELONGS_TO]->(p);

MATCH (p:Project {id: "ijoka-core"})
CREATE (f2:Feature {
  id: randomUUID(),
  description: "MCP server for universal agent interface",
  category: "functional",
  status: "pending",
  priority: 9,
  steps: ["Design MCP tool schema", "Implement stdio transport", "Add tiered tool loading"],
  created_at: datetime(),
  updated_at: datetime(),
  work_count: 0
})-[:BELONGS_TO]->(p);

MATCH (p:Project {id: "ijoka-core"})
CREATE (f3:Feature {
  id: randomUUID(),
  description: "Eliminate feature_list.json - graph is source of truth",
  category: "refactoring",
  status: "pending",
  priority: 8,
  steps: ["Remove JSON file watchers", "Update hooks to use MCP", "Migration for existing projects"],
  created_at: datetime(),
  updated_at: datetime(),
  work_count: 0
})-[:BELONGS_TO]->(p);
