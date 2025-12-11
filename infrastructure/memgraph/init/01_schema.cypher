// ==============================================================================
// IJOKA GRAPH DATABASE SCHEMA
// ==============================================================================
// Memgraph/Neo4j compatible Cypher schema
// Run this on first startup to create indexes and constraints
// ==============================================================================

// ==============================================================================
// CONSTRAINTS (Unique identifiers)
// ==============================================================================

CREATE CONSTRAINT ON (p:Project) ASSERT p.id IS UNIQUE;
CREATE CONSTRAINT ON (p:Project) ASSERT p.path IS UNIQUE;
CREATE CONSTRAINT ON (f:Feature) ASSERT f.id IS UNIQUE;
CREATE CONSTRAINT ON (e:Event) ASSERT e.id IS UNIQUE;
CREATE CONSTRAINT ON (s:Session) ASSERT s.id IS UNIQUE;
CREATE CONSTRAINT ON (i:Insight) ASSERT i.id IS UNIQUE;
CREATE CONSTRAINT ON (r:Rule) ASSERT r.id IS UNIQUE;

// ==============================================================================
// INDEXES (Query performance)
// ==============================================================================

// Project indexes
CREATE INDEX ON :Project(name);
CREATE INDEX ON :Project(created_at);

// Feature indexes
CREATE INDEX ON :Feature(status);
CREATE INDEX ON :Feature(category);
CREATE INDEX ON :Feature(priority);
CREATE INDEX ON :Feature(created_at);

// Event indexes
CREATE INDEX ON :Event(event_type);
CREATE INDEX ON :Event(tool_name);
CREATE INDEX ON :Event(timestamp);
CREATE INDEX ON :Event(success);

// Session indexes
CREATE INDEX ON :Session(status);
CREATE INDEX ON :Session(agent);
CREATE INDEX ON :Session(started_at);
CREATE INDEX ON :Session(is_subagent);

// Insight indexes
CREATE INDEX ON :Insight(pattern_type);
CREATE INDEX ON :Insight(created_at);

// Rule indexes
CREATE INDEX ON :Rule(scope);
CREATE INDEX ON :Rule(enabled);
CREATE INDEX ON :Rule(enforcement);
