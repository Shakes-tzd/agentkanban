#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["neo4j>=5.14.0"]
# ///
"""
Fix worktree attribution - migrate worktree sessions to main project.

Sessions that occurred in worktree directories should be attributed
to the main project, not appear as separate projects.
"""

from neo4j import GraphDatabase

MEMGRAPH_URI = "bolt://localhost:7687"
MAIN_PROJECT_PATH = "/Users/shakes/DevProjects/ijoka"


def fix_worktree_attribution():
    driver = GraphDatabase.driver(MEMGRAPH_URI)

    with driver.session() as session:
        # 1. Find all worktree projects
        print("=== Current Projects ===")
        result = session.run("MATCH (p:Project) RETURN p.path as path, p.name as name ORDER BY p.path")
        projects = list(result)

        worktree_paths = []
        for record in projects:
            path = record["path"]
            name = record["name"]
            is_worktree = "worktrees/task-" in path
            marker = " [WORKTREE]" if is_worktree else ""
            print(f"  - {name}: {path}{marker}")
            if is_worktree:
                worktree_paths.append(path)

        if not worktree_paths:
            print("\nNo worktree projects found - nothing to fix.")
            driver.close()
            return

        # 2. Find sessions in worktree directories
        print(f"\n=== Sessions in Worktrees ({len(worktree_paths)} worktree projects) ===")
        for wt_path in worktree_paths:
            result = session.run("""
                MATCH (s:Session)-[:IN_PROJECT]->(p:Project {path: $path})
                RETURN s.id as session_id, s.agent as agent, s.status as status
            """, path=wt_path)
            sessions = list(result)
            print(f"\n  {wt_path}:")
            if sessions:
                for s in sessions:
                    print(f"    - {s['session_id'][:12]}... ({s['agent']}, {s['status']})")
            else:
                print(f"    (no sessions)")

        # 3. Migrate sessions to main project
        print(f"\n=== Migrating to main project: {MAIN_PROJECT_PATH} ===")

        # Ensure main project exists
        session.run("""
            MERGE (p:Project {path: $path})
            ON CREATE SET p.name = 'ijoka', p.created_at = datetime()
        """, path=MAIN_PROJECT_PATH)

        # For each worktree project, move sessions to main project
        total_migrated = 0
        for wt_path in worktree_paths:
            # Move sessions
            result = session.run("""
                MATCH (s:Session)-[r:IN_PROJECT]->(old:Project {path: $wt_path})
                MATCH (main:Project {path: $main_path})
                DELETE r
                CREATE (s)-[:IN_PROJECT]->(main)
                RETURN count(s) as count
            """, wt_path=wt_path, main_path=MAIN_PROJECT_PATH)
            count = result.single()["count"]
            if count > 0:
                print(f"  Migrated {count} sessions from {wt_path}")
                total_migrated += count

        print(f"\nTotal sessions migrated: {total_migrated}")

        # 4. Delete empty worktree projects
        print("\n=== Cleaning up empty worktree projects ===")
        for wt_path in worktree_paths:
            # Check if project has any remaining relationships
            result = session.run("""
                MATCH (p:Project {path: $path})
                OPTIONAL MATCH (p)-[r]-()
                RETURN count(r) as rel_count
            """, path=wt_path)
            rel_count = result.single()["rel_count"]

            if rel_count == 0:
                session.run("MATCH (p:Project {path: $path}) DELETE p", path=wt_path)
                print(f"  Deleted empty project: {wt_path}")
            else:
                print(f"  Keeping {wt_path} (has {rel_count} relationships)")

        # 5. Verify final state
        print("\n=== Final Project State ===")
        result = session.run("MATCH (p:Project) RETURN p.path as path, p.name as name ORDER BY p.path")
        for record in result:
            print(f"  - {record['name']}: {record['path']}")

    driver.close()
    print("\n✅ Worktree attribution fix complete!")


if __name__ == "__main__":
    fix_worktree_attribution()
